use std::collections::HashMap;
use std::convert::TryInto;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::expr;

use std::fmt;
use std::fmt::Write;

static INIT: &str = "सुरु";

trait Callable {
    fn arity(&self, interpreter: &Interpreter) -> u8;
    fn call(&self, interpreter: &mut Interpreter, args: &[Value]) -> Result<Value, String>;
}

#[derive(Clone)]
pub struct NativeFunction {
    pub name: String,
    pub arity: u8,
    pub callable: fn(&mut Interpreter, &[Value]) -> Result<Value, String>,
}

impl fmt::Debug for NativeFunction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "NativeFunction({})", self.name)
    }
}

impl Callable for NativeFunction {
    fn arity(&self, _interpreter: &Interpreter) -> u8 {
        self.arity
    }
    fn call(&self, interpreter: &mut Interpreter, args: &[Value]) -> Result<Value, String> {
        (self.callable)(interpreter, args)
    }
}

#[derive(Clone, Debug)]
pub struct LoxFunction {
    pub id: u64,
    pub name: expr::Symbol,
    pub parameters: Vec<expr::Symbol>,
    pub body: Vec<expr::Stmt>,
    pub closure: Environment,
    pub this_binding: Option<Box<Value>>,
    pub superclass: Option<u64>,
    pub is_initializer: bool,
}

impl Callable for LoxFunction {
    fn arity(&self, _interpreter: &Interpreter) -> u8 {
        self.parameters.len().try_into().unwrap()
    }
    fn call(&self, interpreter: &mut Interpreter, args: &[Value]) -> Result<Value, String> {
        let args_env: HashMap<_, _> = self
            .parameters
            .iter()
            .zip(args.iter())
            .map(|(param, arg)| {
                (
                    param.name.clone(),
                    (
                        Some(arg.clone()),
                        SourceLocation {
                            line: param.line,
                            col: param.col,
                        },
                    ),
                )
            })
            .collect();

        let saved_env = interpreter.env.clone();
        let saved_retval = interpreter.retval.clone();
        let saved_enclosing_function = interpreter.enclosing_function;

        let mut env = self.closure.clone();
        env.venv.extend(saved_env.venv.clone());
        env.venv.extend(args_env);

        if let Some(this_val) = &self.this_binding {
            // this is just used for lookup on name. source location is meaningless and unused`
            let this_symbol = Interpreter::this_symbol(0, -1);
            env.venv.insert(
                this_symbol.name,
                (
                    Some(*this_val.clone()),
                    SourceLocation {
                        line: this_symbol.line,
                        col: this_symbol.col,
                    },
                ),
            );
        } else if let Ok(this_val) = interpreter.lookup(&Interpreter::this_symbol(0, -1)) {
            // this is just used for lookup on name. source location is meaningless and unused`
            let this_symbol = Interpreter::this_symbol(0, -1);
            env.venv.insert(
                this_symbol.name,
                (
                    Some(this_val.clone()),
                    SourceLocation {
                        line: this_symbol.line,
                        col: this_symbol.col,
                    },
                ),
            );
        }
        let env = env;

        interpreter.env = env;
        interpreter.enclosing_function = Some(self.id);
        interpreter.backtrace.push((0, self.name.name.clone()));
        interpreter.interpret(&self.body)?;

        let retval = interpreter.retval.clone();
        interpreter.backtrace.pop();
        interpreter.enclosing_function = saved_enclosing_function;
        interpreter.env = saved_env;
        interpreter.retval = saved_retval;

        match retval {
            Some(val) => {
                let val_type = type_of(&val);
                if self.is_initializer && val_type != Type::Nil {
                    Err(format!(
                        "TypeError: init should only return nil (perhaps implicitly), not {:?}",
                        val_type
                    ))
                } else {
                    Ok(val)
                }
            }
            None => {
                if self.is_initializer {
                    match &self.this_binding {
                        Some(this_val) => Ok(*this_val.clone()),
                        None => {
                            panic!("Internal intepreter error: could not find binding for this.")
                        }
                    }
                } else {
                    Ok(Value::Nil)
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct LoxClass {
    pub name: expr::Symbol,
    pub superclass: Option<u64>,
    pub id: u64,
    pub methods: HashMap<String, u64>,
}

impl Callable for LoxClass {
    fn arity(&self, interpreter: &Interpreter) -> u8 {
        match self.init(interpreter) {
            Some(initializer) => initializer.parameters.len().try_into().unwrap(),
            None => 0,
        }
    }
    fn call(&self, interpreter: &mut Interpreter, args: &[Value]) -> Result<Value, String> {
        let instance = interpreter.create_instance(&self.name, self.id);

        if let Some(mut initializer) = self.init(interpreter) {
            initializer.this_binding = Some(Box::new(instance.clone()));
            initializer.call(interpreter, args)?;
        }

        Ok(instance)
    }
}

impl LoxClass {
    fn init(&self, interpreter: &Interpreter) -> Option<LoxFunction> {
        self.methods
            .get(&String::from(INIT))
            .map(|initializer_id| interpreter.get_lox_function(*initializer_id).clone())
    }

    fn find_method(
        &self,
        method_name: &str,
        interpreter: &Interpreter,
    ) -> Option<(expr::Symbol, u64)> {
        if let Some(method_id) = self.methods.get(method_name) {
            let lox_fn = interpreter.get_lox_function(*method_id);
            return Some((lox_fn.name.clone(), *method_id));
        } else if let Some(superclass_id) = self.superclass {
            return interpreter
                .get_lox_class(superclass_id)
                .find_method(method_name, interpreter);
        }
        None
    }
}

#[derive(Clone, Debug)]
pub struct LoxInstance {
    pub class_name: expr::Symbol,
    pub class_id: u64,
    pub id: u64,
    pub fields: HashMap<String, Value>,
}

impl LoxInstance {
    fn getattr(&self, attr: &str, interpreter: &Interpreter) -> Result<Value, String> {
        match self.fields.get(attr) {
            Some(val) => Ok(val.clone()),
            None => {
                let cls = interpreter.get_lox_class(self.class_id);
                if let Some((func_name, method_id)) = cls.find_method(attr, interpreter) {
                    return Ok(Value::LoxFunction(
                        func_name,
                        method_id,
                        Some(Box::new(Value::LoxInstance(
                            self.class_name.clone(),
                            self.id,
                        ))),
                    ));
                }
                Err(format!(
                    "AttributeError: '{}' instance has no '{}' attribute.",
                    self.class_name.name, attr
                ))
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    Nil,
    NativeFunction(NativeFunction),
    LoxFunction(
        expr::Symbol,
        /*id*/ u64,
        /*this binding*/ Option<Box<Value>>,
    ),
    LoxClass(expr::Symbol, /*id*/ u64),
    LoxInstance(expr::Symbol, /*id*/ u64),
    List(/*id*/ u64),
}

fn as_callable(interpreter: &Interpreter, value: &Value) -> Option<Box<dyn Callable>> {
    match value {
        Value::NativeFunction(f) => Some(Box::new(f.clone())),
        Value::LoxFunction(_, id, this_binding) => {
            let f = interpreter.get_lox_function(*id);
            let mut f_copy = f.clone();
            f_copy.this_binding = this_binding.clone();
            Some(Box::new(f_copy))
        }
        Value::LoxClass(_, id) => Some(Box::new(interpreter.get_lox_class(*id).clone())),
        _ => None,
    }
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum Type {
    Number,
    String,
    Bool,
    Nil,
    NativeFunction,
    LoxFunction,
    LoxClass,
    LoxInstance,
    List,
}

pub fn type_of(val: &Value) -> Type {
    match val {
        Value::Number(_) => Type::Number,
        Value::String(_) => Type::String,
        Value::Bool(_) => Type::Bool,
        Value::Nil => Type::Nil,
        Value::NativeFunction(_) => Type::NativeFunction,
        Value::LoxFunction(_, _, _) => Type::LoxFunction,
        Value::LoxClass(_, _) => Type::LoxClass,
        Value::LoxInstance(_, _) => Type::LoxInstance,
        Value::List(_) => Type::List,
    }
}

#[derive(Debug, Clone)]
pub struct SourceLocation {
    line: usize,
    col: i64,
}

#[derive(Debug, Default, Clone)]
pub struct Environment {
    enclosing: Option<Box<Environment>>,
    // SourceLocation is the location of a declaration
    venv: HashMap<String, (Option<Value>, SourceLocation)>,
}

pub enum LookupResult<'a> {
    Ok(&'a Value),
    UndefButDeclared(SourceLocation),
    UndefAndNotDeclared,
}

impl Environment {
    pub fn with_enclosing(enclosing: Environment) -> Environment {
        Environment {
            enclosing: Some(Box::new(enclosing)),
            venv: HashMap::new(),
        }
    }

    pub fn define(&mut self, sym: expr::Symbol, maybe_val: Option<Value>) {
        self.venv.insert(
            sym.name,
            (
                maybe_val,
                SourceLocation {
                    line: sym.line,
                    col: sym.col,
                },
            ),
        );
    }

    pub fn lookup(&self, sym: &expr::Symbol) -> LookupResult {
        match self.venv.get(&sym.name) {
            Some((maybe_val, defn_source_location)) => match maybe_val {
                Some(val) => LookupResult::Ok(val),
                None => LookupResult::UndefButDeclared(SourceLocation {
                    line: defn_source_location.line,
                    col: defn_source_location.col,
                }),
            },
            None => LookupResult::UndefAndNotDeclared,
        }
    }

    pub fn get(&self, sym: &expr::Symbol) -> Result<&Value, String> {
        match self.lookup(sym) {
            LookupResult::Ok(val) => Ok(val),
            LookupResult::UndefButDeclared(source_location) => Err(format!(
                "Use of undefined variable '{}' at line={},col={}.\
                \nNote: {} was previously declared at line={},col={}, \
                but was never defined.",
                &sym.name, sym.line, sym.col, &sym.name, source_location.line, source_location.col
            )),
            LookupResult::UndefAndNotDeclared => match &self.enclosing {
                Some(enclosing) => enclosing.get(sym),
                None => Err(format!(
                    "Use of undefined variable {} at line={},col={}.\nNote: {} was never declared.",
                    &sym.name, sym.line, sym.col, &sym.name
                )),
            },
        }
    }

    pub fn assign(&mut self, sym: expr::Symbol, val: &Value) -> Result<(), String> {
        if self.venv.contains_key(&sym.name) {
            self.define(sym, Some(val.clone()));
            return Ok(());
        }

        match &mut self.enclosing {
            Some(enclosing) => enclosing.assign(sym, val),
            None => Err(format!(
                "attempting to assign to undeclared variable at line={},col={}",
                sym.line, sym.col
            )),
        }
    }
}

pub struct Interpreter {
    pub counter: u64,
    pub lambda_counter: u64,
    pub lox_functions: HashMap<u64, LoxFunction>,
    pub lox_instances: HashMap<u64, LoxInstance>,
    pub lox_classes: HashMap<u64, LoxClass>,
    pub lists: HashMap<u64, Vec<Value>>,
    pub env: Environment,
    pub globals: Environment,
    pub retval: Option<Value>,
    pub output: Vec<String>,
    pub enclosing_function: Option<u64>,
    pub interrupted: Arc<AtomicBool>,
    pub backtrace: Vec<(u64, String)>,
}

impl Default for Interpreter {
    fn default() -> Interpreter {
        let mut globals_venv = HashMap::new();
        globals_venv.insert(
            String::from("clock"),
            (
                Some(Value::NativeFunction(NativeFunction {
                    name: String::from("clock"),
                    arity: 0,
                    callable: |_, _| {
                        let start = SystemTime::now();
                        let since_the_epoch = start.duration_since(UNIX_EPOCH).unwrap();

                        Ok(Value::Number(since_the_epoch.as_millis() as f64))
                    },
                })),
                SourceLocation {
                    line: 1337,
                    col: 1337,
                },
            ),
        );
        globals_venv.insert(
            String::from("len"),
            (
                Some(Value::NativeFunction(NativeFunction {
                    name: String::from("len"),
                    arity: 1,
                    callable: |interp, values| match &values[0] {
                        Value::String(s) => Ok(Value::Number(s.len() as f64)),
                        Value::List(list_id) => {
                            let elts = interp.get_list_elts(*list_id);
                            Ok(Value::Number(elts.len() as f64))
                        }
                        val => Err(format!("Object of type {:?} has no len.", type_of(val))),
                    },
                })),
                SourceLocation {
                    line: 1337,
                    col: 1337,
                },
            ),
        );
        globals_venv.insert(
            String::from("iota"),
            (
                Some(Value::NativeFunction(NativeFunction {
                    name: String::from("iota"),
                    arity: 2,
                    callable: |interpreter, values| match (&values[0], &values[1]) {
                        (Value::Number(low), Value::Number(high)) => {
                            let elts: Vec<_> = (*low as i64..*high as i64)
                                .map(|x| Value::Number(x as f64))
                                .collect();
                            Ok(interpreter.create_list(elts))
                        }
                        (Value::Number(_), high) => Err(format!(
                            "invalid high argument of type {:?} in iota expression.",
                            type_of(high)
                        )),
                        (low, _) => Err(format!(
                            "invalid low argument of type {:?} in iota expression.",
                            type_of(low)
                        )),
                    },
                })),
                SourceLocation {
                    line: 1337,
                    col: 1337,
                },
            ),
        );

        globals_venv.insert(
            String::from("forEach"),
            (
                Some(Value::NativeFunction(NativeFunction {
                    name: String::from("forEach"),
                    arity: 2,
                    callable: |interpreter, values| match &values[0] {
                        Value::List(list_id) => {
                            let elts = interpreter.get_list_elts(*list_id).clone();
                            let maybe_callable = as_callable(interpreter, &values[1]);
                            match maybe_callable {
                                Some(callable) => {
                                    for elt in elts {
                                        callable.call(interpreter, &[elt])?;
                                    }
                                    Ok(Value::Nil)
                                }
                                None => Err(format!(
                                    "The second argument to for_each must be callable. Found {:?}.",
                                    type_of(&values[1])
                                )),
                            }
                        }
                        val => Err(format!(
                            "Can't call forEach on value of type {:?}.",
                            type_of(val)
                        )),
                    },
                })),
                SourceLocation {
                    line: 1337,
                    col: 1337,
                },
            ),
        );
        globals_venv.insert(
            String::from("map"),
            (
                Some(Value::NativeFunction(NativeFunction {
                    name: String::from("map"),
                    arity: 2,
                    callable: |interpreter, values| match &values[1] {
                        Value::List(list_id) => {
                            let maybe_callable = as_callable(interpreter, &values[0]);
                            match maybe_callable {
                                Some(callable) => {
                                    let mut res_elts = Vec::new();
                                    let elts = interpreter.get_list_elts(*list_id).clone();
                                    for elt in elts {
                                        res_elts.push(callable.call(interpreter, &[elt.clone()])?);
                                    }
                                    Ok(interpreter.create_list(res_elts))
                                }
                                None => Err(format!(
                                    "The second argument to for_each must be callable. Found {:?}.",
                                    type_of(&values[1])
                                )),
                            }
                        }
                        val => Err(format!(
                            "Can't call forEach on value of type {:?}.",
                            type_of(val)
                        )),
                    },
                })),
                SourceLocation {
                    line: 1337,
                    col: 1337,
                },
            ),
        );

        let globals = Environment {
            enclosing: None,
            venv: globals_venv,
        };

        Interpreter {
            counter: 0,
            lambda_counter: 0,
            lox_functions: Default::default(),
            lox_instances: Default::default(),
            lox_classes: Default::default(),
            lists: Default::default(),
            env: Default::default(),
            globals,
            retval: None,
            output: Default::default(),
            enclosing_function: None,
            interrupted: Arc::new(AtomicBool::new(false)),
            backtrace: vec![(0, "script".to_string())],
        }
    }
}

impl Interpreter {
    pub fn interpret(&mut self, stmts: &[expr::Stmt]) -> Result<(), String> {
        self.interrupted.store(false, Ordering::Release);
        for stmt in stmts {
            self.execute(stmt)?
        }
        Ok(())
    }

    pub fn get_lox_function(&self, id: u64) -> &LoxFunction {
        match self.lox_functions.get(&id) {
            Some(func) => func,
            None => panic!(
                "Internal interpreter error! couldn't find an function with id {}.",
                id
            ),
        }
    }

    pub fn get_lox_class(&self, id: u64) -> &LoxClass {
        match self.lox_classes.get(&id) {
            Some(func) => func,
            None => panic!(
                "Internal interpreter error! couldn't find class with id {}.",
                id
            ),
        }
    }

    pub fn get_lox_instance(&self, id: u64) -> &LoxInstance {
        match self.lox_instances.get(&id) {
            Some(inst) => inst,
            None => panic!(
                "Internal interpreter error: could not find an instance with id {}.",
                id
            ),
        }
    }

    pub fn format_backtrace(&self) -> String {
        let lines: Vec<_> = self
            .backtrace
            .iter()
            .map(|(_, funname)| format!("[line ??] in {}", funname))
            .collect();
        format!("Backtrace (most recent call last):\n\n{}", lines.join("\n"))
    }

    fn get_list_elts(&self, list_id: u64) -> &Vec<Value> {
        if let Some(elts) = self.lists.get(&list_id) {
            elts
        } else {
            panic!(
                "Internal interpreter error! Couldn't find list with id {}.",
                list_id
            );
        }
    }


    fn alloc_id(&mut self) -> u64 {
        let res = self.counter;
        self.counter += 1;
        res
    }

    fn create_list(&mut self, elts: Vec<Value>) -> Value {
        let list_id = self.alloc_id();
        self.lists.insert(list_id, elts);
        Value::List(list_id)
    }

    fn create_instance(&mut self, class_name: &expr::Symbol, class_id: u64) -> Value {
        let inst_id = self.alloc_id();
        let inst = LoxInstance {
            class_name: class_name.clone(),
            class_id,
            id: inst_id,
            fields: HashMap::new(),
        };
        self.lox_instances.insert(inst_id, inst);
        Value::LoxInstance(class_name.clone(), inst_id)
    }

    fn execute(&mut self, stmt: &expr::Stmt) -> Result<(), String> {
        if self.retval.is_some() {
            return Ok(());
        }

        match stmt {
            expr::Stmt::Expr(e) => match self.interpret_expr(e) {
                Ok(_) => Ok(()),
                Err(err) => Err(err),
            },
            expr::Stmt::ClassDecl(expr::ClassDecl {
                name: sym,
                superclass: maybe_superclass,
                methods: stmt_methods,
            }) => {
                let class_id = self.alloc_id();
                self.env
                    .define(sym.clone(), Some(Value::LoxClass(sym.clone(), class_id)));

                let superclass_id = if let Some(superclass_var) = maybe_superclass {
                    if superclass_var.name == sym.name {
                        return Err(format!(
                            "A class cannot inerit from itself (line={}, col={})",
                            sym.line, sym.col
                        ));
                    }

                    let superclass_val =
                        self.interpret_expr(&expr::Expr::Variable(superclass_var.clone()))?;
                    if let Value::LoxClass(_, id) = superclass_val {
                        Some(id)
                    } else {
                        return Err(format!(
                            "Only classes should appear as superclasses. Found {:?}.",
                            type_of(&superclass_val)
                        ));
                    }
                } else {
                    None
                };

                let mut methods = HashMap::new();
                for method in stmt_methods.iter() {
                    let func_id = self.alloc_id();

                    methods.insert(method.name.name.clone(), func_id);

                    let is_initializer = method.name.name == INIT;

                    let lox_function = LoxFunction {
                        id: func_id,
                        name: method.name.clone(),
                        parameters: method.params.clone(),
                        body: method.body.clone(),
                        closure: self.env.clone(),
                        this_binding: None,
                        superclass: superclass_id,
                        is_initializer,
                    };

                    self.lox_functions.insert(func_id, lox_function);
                }

                let cls = LoxClass {
                    name: sym.clone(),
                    superclass: superclass_id,
                    id: class_id,
                    methods,
                };

                self.lox_classes.insert(class_id, cls);
                Ok(())
            }
            expr::Stmt::FunDecl(expr::FunDecl {
                name,
                params: parameters,
                body,
            }) => {
                let func_id = self.alloc_id();
                self.env.define(
                    name.clone(),
                    Some(Value::LoxFunction(name.clone(), func_id, None)),
                );

                let lox_function = LoxFunction {
                    id: func_id,
                    name: name.clone(),
                    parameters: parameters.clone(),
                    body: body.clone(),
                    closure: self.env.clone(),
                    this_binding: None,
                    superclass: None,
                    is_initializer: false,
                };

                self.lox_functions.insert(func_id, lox_function);

                Ok(())
            }
            expr::Stmt::If(cond, if_true, maybe_if_false) => {
                if Interpreter::is_truthy(&self.interpret_expr(cond)?) {
                    return self.execute(if_true);
                }
                if let Some(if_false) = maybe_if_false {
                    return self.execute(if_false);
                }
                Ok(())
            }
            expr::Stmt::Print(e) => match self.interpret_expr(e) {
                Ok(val) => {
                    println!("{}", self.format_val(&val));
                    self.output.push(self.format_val(&val));
                    Ok(())
                }
                Err(err) => Err(err),
            },
            expr::Stmt::VarDecl(sym, maybe_expr) => {
                let maybe_val = match maybe_expr {
                    Some(expr) => Some(self.interpret_expr(expr)?),
                    None => None,
                };
                self.env.define(sym.clone(), maybe_val);
                Ok(())
            }
            expr::Stmt::Block(stmts) => {
                self.env = Environment::with_enclosing(self.env.clone());

                for stmt in stmts.iter() {
                    self.execute(stmt)?;
                }

                if let Some(enclosing) = self.env.enclosing.clone() {
                    self.env = *enclosing
                } else {
                    // TODO: how to do this without a runtime check?
                    panic!("impossible");
                }

                Ok(())
            }
            expr::Stmt::While(cond, body) => {
                while Interpreter::is_truthy(&self.interpret_expr(cond)?) {
                    self.execute(body)?;
                }
                Ok(())
            }
            expr::Stmt::Return(_, maybe_res) => {
                self.retval = Some(if let Some(res) = maybe_res {
                    self.interpret_expr(res)?
                } else {
                    Value::Nil
                });
                Ok(())
            }
        }
    }

    fn lookup(&self, sym: &expr::Symbol) -> Result<&Value, String> {
        match self.env.get(sym) {
            Ok(val) => Ok(val),
            Err(_) => self.globals.get(sym),
        }
    }

    fn this_symbol(line: usize, col: i64) -> expr::Symbol {
        expr::Symbol {
            name: String::from("this"),
            line,
            col,
        }
    }

    fn interpret_expr(&mut self, expr: &expr::Expr) -> Result<Value, String> {
        if self.interrupted.load(Ordering::Acquire) {
            return Ok(Value::Nil);
        }

        match expr {
            expr::Expr::This(source_location) => match self.lookup(&Interpreter::this_symbol(
                source_location.line,
                source_location.col,
            )) {
                Ok(val) => Ok(val.clone()),
                Err(err) => Err(err),
            },
            expr::Expr::Literal(lit) => Ok(Interpreter::interpret_literal(lit)),
            expr::Expr::Unary(op, e) => self.interpret_unary(*op, e),
            expr::Expr::Binary(lhs, op, rhs) => self.interpret_binary(lhs, *op, rhs),
            expr::Expr::Call(callee, loc, args) => self.call(callee, loc, args),
            expr::Expr::Get(lhs, attr) => self.getattr(lhs, &attr.name),
            expr::Expr::Set(lhs, attr, rhs) => self.setattr(lhs, attr, rhs),
            expr::Expr::Grouping(e) => self.interpret_expr(e),
            expr::Expr::Variable(sym) => match self.lookup(sym) {
                Ok(val) => Ok(val.clone()),
                Err(err) => Err(err),
            },
            expr::Expr::Assign(sym, val_expr) => {
                let val = self.interpret_expr(val_expr)?;

                self.env.assign(sym.clone(), &val)?;

                Ok(val)
            }
            expr::Expr::Logical(left_expr, expr::LogicalOp::Or, right_expr) => {
                let left = self.interpret_expr(left_expr)?;
                if Interpreter::is_truthy(&left) {
                    Ok(left)
                } else {
                    Ok(self.interpret_expr(right_expr)?)
                }
            }
            expr::Expr::Logical(left_expr, expr::LogicalOp::And, right_expr) => {
                let left = self.interpret_expr(left_expr)?;
                if !Interpreter::is_truthy(&left) {
                    Ok(left)
                } else {
                    Ok(self.interpret_expr(right_expr)?)
                }
            }
            expr::Expr::Super(source_location, sym) => match self.enclosing_function {
                Some(func_id) => {
                    let func = self.get_lox_function(func_id);
                    match &func.superclass {
                        Some(superclass_id) => {
                            let superclass = self.get_lox_class(*superclass_id);
                            if let Some((func_name, method_id)) = superclass.find_method(&sym.name, self) {
                                let method = self.get_lox_function(method_id);
                                Ok(Value::LoxFunction(
                                    func_name,
                                    method.id,
                                    func.this_binding.clone()))
                            }
                            else {
                                Err(format!("no superclass has method {} at line={}, col={}",
                                            sym.name, source_location.line, source_location.col))
                            }
                        }
                        _ => {
                            Err(format!("Super expression not enclosed in a method definition at line={}, col={}.",
                                        source_location.line, source_location.col))
                        }
                    }
                }
                None => Err(format!(
                    "super expression not enclosed in a function at line={}, col={}.",
                    source_location.line, source_location.col
                )),
            },
        }
    }


    fn getattr(&mut self, lhs: &expr::Expr, attr: &str) -> Result<Value, String> {
        let val = self.interpret_expr(lhs)?;
        match val {
            Value::LoxInstance(_, id) => self.get_lox_instance(id).getattr(attr, self),
            _ => Err(format!(
                "Only LoxInstance values have attributes. Found {:?}.",
                type_of(&val)
            )),
        }
    }

    fn setattr(
        &mut self,
        lhs_exp: &expr::Expr,
        attr: &expr::Symbol,
        rhs_exp: &expr::Expr,
    ) -> Result<Value, String> {
        let lhs = self.interpret_expr(lhs_exp)?;
        let rhs = self.interpret_expr(rhs_exp)?;
        match lhs {
            Value::LoxInstance(_, id) => match self.lox_instances.get_mut(&id) {
                Some(inst) => {
                    inst.fields.insert(attr.name.clone(), rhs.clone());
                    Ok(rhs)
                }
                None => panic!(
                    "Internal interpreter error: could not find instance with id {}",
                    id
                ),
            },
            _ => Err(format!(
                "Only LoxInstance values have attributes. Found {:?}.",
                type_of(&lhs)
            )),
        }
    }

    fn call(
        &mut self,
        callee_expr: &expr::Expr,
        loc: &expr::SourceLocation,
        arg_exprs: &[expr::Expr],
    ) -> Result<Value, String> {
        let callee = self.interpret_expr(callee_expr)?;

        match as_callable(self, &callee) {
            Some(callable) => {
                let maybe_args: Result<Vec<_>, _> = arg_exprs
                    .iter()
                    .map(|arg| self.interpret_expr(arg))
                    .collect();

                match maybe_args {
                    Ok(args) => {
                        if args.len() != callable.arity(self).into() {
                            Err(format!(
                                "Invalid call at line={},col={}: callee has arity {}, but \
                                         was called with {} arguments",
                                loc.line,
                                loc.col,
                                callable.arity(self),
                                args.len()
                            ))
                        } else {
                            callable.call(self, &args)
                        }
                    }
                    Err(err) => Err(err),
                }
            }
            None => Err(format!(
                "value {:?} is not callable at line={},col={}",
                callee, loc.line, loc.col
            )),
        }
    }

    fn interpret_binary(
        &mut self,
        lhs_expr: &expr::Expr,
        op: expr::BinaryOp,
        rhs_expr: &expr::Expr,
    ) -> Result<Value, String> {
        let lhs = self.interpret_expr(lhs_expr)?;
        let rhs = self.interpret_expr(rhs_expr)?;

        match (&lhs, op.ty, &rhs) {
            (Value::Number(n1), expr::BinaryOpTy::Less, Value::Number(n2)) => {
                Ok(Value::Bool(n1 < n2))
            }
            (Value::Number(n1), expr::BinaryOpTy::LessEqual, Value::Number(n2)) => {
                Ok(Value::Bool(n1 <= n2))
            }
            (Value::Number(n1), expr::BinaryOpTy::Greater, Value::Number(n2)) => {
                Ok(Value::Bool(n1 > n2))
            }
            (Value::Number(n1), expr::BinaryOpTy::GreaterEqual, Value::Number(n2)) => {
                Ok(Value::Bool(n1 >= n2))
            }
            (Value::Number(n1), expr::BinaryOpTy::Plus, Value::Number(n2)) => {
                Ok(Value::Number(n1 + n2))
            }
            (Value::Number(n1), expr::BinaryOpTy::Minus, Value::Number(n2)) => {
                Ok(Value::Number(n1 - n2))
            }
            (Value::Number(n1), expr::BinaryOpTy::Star, Value::Number(n2)) => {
                Ok(Value::Number(n1 * n2))
            }
            (Value::Number(n1), expr::BinaryOpTy::Slash, Value::Number(n2)) => {
                if *n2 != 0.0 {
                    Ok(Value::Number(n1 / n2))
                } else {
                    Err(format!(
                        "division by zero at line={},col={}",
                        op.line, op.col
                    ))
                }
            }
            (Value::String(s1), expr::BinaryOpTy::Plus, Value::String(s2)) => {
                Ok(Value::String(format!("{}{}", s1, s2)))
            }
            (Value::List(xs_id), expr::BinaryOpTy::Plus, Value::List(ys_id)) => {
                let xs = self.get_list_elts(*xs_id);
                let ys = self.get_list_elts(*ys_id);
                let mut res = xs.clone();
                res.extend(ys.clone());
                Ok(self.create_list(res))
            }
            (_, expr::BinaryOpTy::EqualEqual, _) => {
                Ok(Value::Bool(Interpreter::equals(&lhs, &rhs)))
            }
            (_, expr::BinaryOpTy::NotEqual, _) => Ok(Value::Bool(!Interpreter::equals(&lhs, &rhs))),
            _ => Err(format!(
                "invalid operands in binary operator {:?} of type {:?} and {:?} at line={},col={}",
                op.ty,
                type_of(&lhs),
                type_of(&rhs),
                op.line,
                op.col
            )),
        }
    }

    fn equals(lhs: &Value, rhs: &Value) -> bool {
        match (lhs, rhs) {
            (Value::Number(n1), Value::Number(n2)) => (n1 - n2).abs() < f64::EPSILON,
            (Value::String(s1), Value::String(s2)) => s1 == s2,
            (Value::Bool(b1), Value::Bool(b2)) => b1 == b2,
            (Value::Nil, Value::Nil) => true,
            (_, _) => false,
        }
    }

    fn interpret_unary(&mut self, op: expr::UnaryOp, expr: &expr::Expr) -> Result<Value, String> {
        let val = self.interpret_expr(expr)?;

        match (op.ty, &val) {
            (expr::UnaryOpTy::Minus, Value::Number(n)) => Ok(Value::Number(-n)),
            (expr::UnaryOpTy::Bang, _) => Ok(Value::Bool(!Interpreter::is_truthy(&val))),
            (_, Value::String(_)) => Err(format!(
                "invalid application of unary op {:?} to object of type String at line={},col={}",
                op.ty, op.line, op.col
            )),
            (_, Value::NativeFunction(_)) => Err(format!(
                "invalid application of unary op {:?} to object of type NativeFunction at line={},col={}",
                op.ty, op.line, op.col
            )),
            (_, Value::LoxFunction(_, _, _)) => Err(format!(
                "invalid application of unary op {:?} to object of type LoxFunction at line={},col={}",
                op.ty, op.line, op.col
            )),
            (_, Value::LoxClass(_, _)) => Err(format!(
                "invalid application of unary op {:?} to object of type LoxClass at line={},col={}",
                op.ty, op.line, op.col
            )),
            (_, Value::LoxInstance(class_name, _)) => Err(format!(
                "invalid application of unary op {:?} to object of type {:?} at line={},col={}",
                class_name.name, op.ty, op.line, op.col
            )),
            (expr::UnaryOpTy::Minus, Value::Bool(_)) => Err(format!(
                "invalid application of unary op {:?} to object of type Bool at line={},col={}",
                op.ty, op.line, op.col
            )),
            (_, Value::Nil) => Err(format!(
                "invalid application of unary op {:?} to nil at line={},col={}",
                op.ty, op.line, op.col
            )),
            (_, Value::List(_)) => Err(format!(
                "invalid application of unary op {:?} to list at line={},col={}",
                op.ty, op.line, op.col
            )),
        }
    }

    fn is_truthy(val: &Value) -> bool {
        match val {
            Value::Nil => false,
            Value::Bool(b) => *b,
            _ => true,
        }
    }

    fn interpret_literal(lit: &expr::Literal) -> Value {
        match lit {
            expr::Literal::Number(n) => Value::Number(*n),
            expr::Literal::String(s) => Value::String(s.clone()),
            expr::Literal::True => Value::Bool(true),
            expr::Literal::False => Value::Bool(false),
            expr::Literal::Nil => Value::Nil,
        }
    }

    fn format_val(&self, val: &Value) -> String {
        match val {
            Value::Number(n) => format!("{}", n),
            Value::String(s) => format!("'{}'", s),
            Value::Bool(b) => format!("{}", b),
            Value::Nil => "nil".to_string(),
            Value::NativeFunction(func) => format!("NativeFunction({})", func.name),
            Value::LoxFunction(sym, _, _) => format!("LoxFunction({})", sym.name),
            Value::LoxClass(sym, _) => format!("LoxClass({})", sym.name),
            Value::LoxInstance(sym, _) => format!("LoxInstance({})", sym.name),
            Value::List(list_id) => {
                let mut res = String::new();
                write!(&mut res, "[").unwrap();
                let elements = self.get_list_elts(*list_id);
                elements.split_last().map(|(last_elt, rest)| {
                    rest.iter()
                        .try_for_each(|elt| write!(&mut res, "{}, ", self.format_val(elt)))
                        .unwrap();
                    write!(&mut res, "{}", self.format_val(last_elt))
                });
                write!(&mut res, "]").unwrap();
                res
            }
        }
    }
}
