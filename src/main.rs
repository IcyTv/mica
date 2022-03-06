#![allow(clippy::vec_box)]

use std::path::PathBuf;
use std::rc::Rc;

use bytecode::{Chunk, Environment, Function, FunctionKind};
use codegen::CodeGenerator;
use common::{Error, ErrorKind};
use rustyline::completion::Completer;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Editor, Helper};

use lexer::Lexer;
use parser::Parser;
use structopt::StructOpt;
use value::{Closure, Value};
use vm::{Fiber, Globals};

mod ast;
mod bytecode;
mod codegen;
mod common;
mod lexer;
mod parser;
mod value;
mod vm;

#[derive(StructOpt)]
#[structopt(name = "mica")]
struct Opts {
   file: Option<PathBuf>,
}

struct MicaValidator;

impl Helper for MicaValidator {}

impl Hinter for MicaValidator {
   type Hint = String;
}

impl Highlighter for MicaValidator {}

impl Completer for MicaValidator {
   type Candidate = String;
}

impl Validator for MicaValidator {
   fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
      let lexer = Lexer::new(Rc::from(""), ctx.input().to_owned());
      let parser = Parser::new(lexer);
      if let Err(error) = parser.parse() {
         if let Error::Compile {
            kind:
               ErrorKind::MissingClosingQuote | ErrorKind::MissingEnd | ErrorKind::MissingRightParen,
            ..
         } = error
         {
            return Ok(ValidationResult::Incomplete);
         }
      }
      Ok(ValidationResult::Valid(None))
   }
}

fn compile(globals: &mut Environment, filename: &str, input: String) -> Result<Rc<Chunk>, Error> {
   let module_name = Rc::from(filename);
   let lexer = Lexer::new(Rc::clone(&module_name), input);
   let parser = Parser::new(lexer);
   let (ast, root_node) = parser.parse()?;
   CodeGenerator::new(module_name, globals).generate(&ast, root_node)
}

fn interpret(
   (env, globals): (&mut Environment, &mut Globals),
   filename: &str,
   input: String,
) -> Option<Value> {
   let chunk = match compile(env, filename, input) {
      Ok(chunk) => chunk,
      Err(error) => {
         eprintln!("{error}");
         return None;
      }
   };
   println!("{env:?}");
   println!("{chunk:?}");
   let mut fiber = Fiber::new(chunk);
   let result = match fiber.interpret(env, globals) {
      Ok(value) => value,
      Err(error) => {
         eprintln!("{error}");
         return None;
      }
   };
   Some(result)
}

fn env() -> (Environment, Globals) {
   let mut env = Environment::new();
   let mut globals = Globals::new();

   let print_id = env
      .create_function(Function {
         name: Rc::from("print"),
         parameter_count: 1,
         kind: FunctionKind::Foreign(|args| {
            for arg in args {
               print!("{}", arg);
            }
            println!();
            Value::Nil
         }),
      })
      .unwrap();
   let print_global = env.create_global("print").unwrap();
   globals.set(
      print_global,
      Value::Function(Rc::new(Closure {
         function_id: print_id,
      })),
   );

   (env, globals)
}

fn repl() {
   let mut editor =
      Editor::with_config(rustyline::Config::builder().auto_add_history(true).build());
   editor.set_helper(Some(MicaValidator));
   let (mut env, mut globals) = env();
   while let Ok(line) = editor.readline("> ") {
      if let Some(result) = interpret((&mut env, &mut globals), "(repl)", line) {
         println!("< {result:?}");
         println!();
      }
   }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
   let opts = Opts::from_args();
   if let Some(path) = &opts.file {
      let file = std::fs::read_to_string(path)?;
      let (mut env, mut globals) = env();
      let _ = interpret((&mut env, &mut globals), path.to_str().unwrap(), file);
   } else {
      repl();
   }

   Ok(())
}
