# Nep-Preter in Rust

This interpreter was written in accordance to Crafting Interpreters Book. The scanner is Recursive Descent and the interpreter type is Tree walk interpreter

The Keywords are

```
 "अनि", "वर्ग", "अरु", "गलत", "भुमरी", "कार्य", "यदि", "आलु", "वा", "छाप", "रिटन", "सुपर", "यो", "सहि", "भार", "जबसम्म"
```

Steps to run the program

```

cargo run -- filename

for more information

cargo run -- -h


```

## Some Examples

```
भार क = सहि;
छाप क;

काम परीक्षण() {
    रिटन "प्रोगामिङ";
}

छाप परीक्षण();
वर्ग चित्र {
     सुरु(क, ख){
        यो.क = क;
        यो.ख = ख;
    }
    खिच() {
       छाप "खिच"+" फोटो " + यो.क;
       छाप यो.ख;
    }
}
भार स = चित्र("test" ,3);
स.खिच();
```

For loop and if cases

```
छाप "nepal";
भार a = १२;

यदि (a == १२) {
    छाप a;
}
भुमरी (भार i = १; i < १० ; i=i+१){
    छाप i;
}
```
