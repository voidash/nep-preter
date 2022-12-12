extern crate clap;

use clap::{App, Arg};

use std::fs;

mod expr;
mod input;
mod parser;
mod scanner;
mod interpreter;

const FILENAME: &str = "file";
const INPUT: &str = "c";

fn get_input(matches: &clap::ArgMatches<'_>) -> Option<input::Input> {
    if let Some(literal_input) = matches.value_of(INPUT) {
        return Some(input::Input {
            source: input::Source::Literal,
            content: literal_input.to_string(),
        });
    }

    if let Some(input_file) = matches.value_of(FILENAME) {
        match fs::read_to_string(input_file) {
            Ok(input) => {
                return Some(input::Input {
                    source: input::Source::File(input_file.to_string()),
                    content: nepfy(input),
                });
            }
            Err(err) => {
                panic!("Error reading {}: {}", input_file, err);
            }
        }
    }

    None
}
fn nepfy(data: String) -> String {
    data.chars().collect::<Vec<char>>().iter().map(|f| {
        let nepali_numbers = ['०','१','२','३','४','५','६','७','८','९'];
        if let Some(pos) = nepali_numbers.iter().position(|d|  d == f ) {
            return char::from_digit(pos as u32,10).unwrap();
        }
        *f
    }).collect::<String>()
}

fn main() {


    let matches = App::new("loxi")
        .version("1.0")
        .about("नेप-Preter")
        .author("आशिष थापा").arg(
            Arg::with_name(FILENAME)
                .help("फाइल नेम दिनुहोस")
                .required(false)
                .index(1),
        )
        .arg(
            Arg::with_name(INPUT)
                .long("-c")
                .takes_value(true)
                .help("यहि स्ट्रिङ पास गर्नुहोस"),
        )
        .get_matches();


    if let Some(input) = get_input(&matches) {
            match scanner::scan_tokens(input.content.clone()) {
                Ok(tokens) => {
                    // println!("{:#?}", tokens);

                    let stmts_maybe = parser::parse(tokens);

                    match stmts_maybe {
                        Ok(stmts) => {
                            // println!("{:#?}", stmts);
                            let mut interpreter: interpreter::Interpreter =
                                Default::default();
                            let interpret_result = interpreter.interpret(&stmts);

                            match interpret_result {
                                Ok(_) => {
                                    std::process::exit(0);
                                }
                                Err(err) => {
                                    println!(
                                        "Runtime Error: {}\n\n{}",
                                        err,
                                        interpreter.format_backtrace()
                                    );
                                    std::process::exit(-1);
                                }
                            }
                        }
                        Err(err) => {
                            panic!("{:?} - {:?}",err,&input);
                        }
                    }
                }
                Err(err) => {
                    panic!("{:?} - {:?}",err,&input);
                }
            }
        

    }

}