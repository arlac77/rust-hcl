#![cfg(feature = "alloc")]

extern crate nom;


use nom::{
  branch::alt,
  bytes::complete::{escaped, tag, take_while},
  character::complete::{alphanumeric1 as alphanumeric, char, one_of},
  combinator::{map, opt, cut},
  error::{context, convert_error, ErrorKind, ParseError,VerboseError},
  multi::separated_list,
  number::complete::double,
  sequence::{delimited, preceded, separated_pair, terminated},
  Err, IResult,
};
use std::collections::HashMap;
use std::str;

#[derive(Debug, PartialEq)]
pub enum JsonValue {
  Str(String),
  Boolean(bool),
  Num(f64),
  Array(Vec<JsonValue>),
  Object(HashMap<String, JsonValue>),
}

/// parser combinators are constructed from the bottom up:
/// first we write parsers for the smallest elements (here a space character),
/// then we'll combine them in larger parsers
fn sp<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
  let chars = " \t\r\n";

  // nom combinators like `take_while` return a function. That function is the
  // parser,to which we can pass the input
  take_while(move |c| chars.contains(c))(i)
}

/// A nom parser has the following signature:
/// `Input -> IResult<Input, Output, Error>`, with `IResult` defined as:
/// `type IResult<I, O, E = (I, ErrorKind)> = Result<(I, O), Err<E>>;`
///
/// most of the times you can ignore the error type and use the default (but this
/// examples shows custom error types later on!)
///
/// Here we use `&str` as input type, but nom parsers can be generic over
/// the input type, and work directly with `&[u8]` or any other type that
/// implements the required traits.
///
/// Finally, we can see here that the input and output type are both `&str`
/// with the same lifetime tag. This means that the produced value is a subslice
/// of the input data. and there is no allocation needed. This is the main idea
/// behind nom's performance.
fn parse_str<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
  escaped(alphanumeric, '\\', one_of("\"n\\"))(i)
}

/// `tag(string)` generates a parser that recognizes the argument string.
///
/// we can combine it with other functions, like `map` that takes the result
/// of another parser, and applies a function over it (`map` itself generates
/// a new parser`.
///
/// `alt` is another combinator that tries multiple parsers one by one, until
/// one of them succeeds
fn boolean<'a, E: ParseError<&'a str>>(input: &'a str) -> IResult<&'a str, bool, E> {
  alt((
      map(tag("false"), |_| false),
      map(tag("true"), |_| true)
  ))(input)
}

/// this parser combines the previous `parse_str` parser, that recognizes the
/// interior of a string, with a parse to recognize the double quote character,
/// before the string (using `preceded`) and after the string (using `terminated`).
///
/// `context` and `cut` are related to error management:
/// - `cut` transforms an `Err::Error(e)` in `Err::Failure(e)`, signaling to
/// combinators like  `alt` that they should not try other parsers. We were in the
/// right branch (since we found the `"` character) but encountered an error when
/// parsing the string
/// - `context` lets you add a static string to provide more information in the
/// error chain (to indicate which parser had an error)
fn string<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, &'a str, E> {
  context("string",
    preceded(
      char('\"'),
      cut(terminated(
          parse_str,
          char('\"')
  ))))(i)
}

/// some combinators, like `separated_list` or `many0`, will call a parser repeatedly,
/// accumulating results in a `Vec`, until it encounters an error.
/// If you want more control on the parser application, check out the `iterator`
/// combinator (cf `examples/iterator.rs`)
fn array<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, Vec<JsonValue>, E> {
  context(
    "array",
    preceded(char('['),
    cut(terminated(
      separated_list(preceded(sp, char(',')), value),
      preceded(sp, char(']'))))
  ))(i)
}

fn key_value<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, (&'a str, JsonValue), E> {
separated_pair(preceded(sp, string), cut(preceded(sp, char(':'))), value)(i)
}

fn hash<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, HashMap<String, JsonValue>, E> {
  context(
    "map",
    preceded(char('{'),
    cut(terminated(
      map(
        separated_list(preceded(sp, char(',')), key_value),
        |tuple_vec| {
          tuple_vec.into_iter().map(|(k, v)| (String::from(k), v)).collect()
      }),
      preceded(sp, char('}')),
    ))
  ))(i)
}

/// here, we apply the space parser before trying to parse a value
fn value<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, JsonValue, E> {
  preceded(
    sp,
    alt((
      map(hash, JsonValue::Object),
      map(array, JsonValue::Array),
      map(string, |s| JsonValue::Str(String::from(s))),
      map(double, JsonValue::Num),
      map(boolean, JsonValue::Boolean),
    )),
  )(i)
}

/// the root element of a JSON parser is either an object or an array
fn root<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, JsonValue, E> {
  delimited(
    sp,
    alt((map(hash, JsonValue::Object), map(array, JsonValue::Array))),
    opt(sp),
  )(i)
}

fn main() {
  let data = "  { \"a\"\t: 42,
  \"b\": [ \"x\", \"y\", 12 ] ,
  \"c\": { \"hello\" : \"world\"
  }
  } ";

  println!("will try to parse valid JSON data:\n\n**********\n{}\n**********\n", data);

  println!(
    "parsing a valid file:\n{:#?}\n",
    root::<(&str, ErrorKind)>(data)
  );

  let data = "  { \"a\"\t: 42,
  \"b\": [ \"x\", \"y\", 12 ] ,
  \"c\": { 1\"hello\" : \"world\"
  }
  } ";

  println!("will try to parse invalid JSON data:\n\n**********\n{}\n**********\n", data);

  // here we use `(Input, ErrorKind)` as error type, which is used by default
  // if you don't specify it. It contains the position of the error and some
  // info on which parser encountered it.
  // It is fast and small, but does not provide much context.
  //
  // This will print:
  // basic errors - `root::<(&str, ErrorKind)>(data)`:
  // Err(
  //   Failure(
  //       (
  //           "1\"hello\" : \"world\"\n  }\n  } ",
  //           Char,
  //       ),
  //   ),
  // )
  println!(
    "basic errors - `root::<(&str, ErrorKind)>(data)`:\n{:#?}\n",
    root::<(&str, ErrorKind)>(data)
  );

  // nom also provides `the `VerboseError<Input>` type, which will generate a sort
  // of backtrace of the path through the parser, accumulating info on input positions
  // and affected parsers.
  //
  // This will print:
  //
  // parsed verbose: Err(
  //   Failure(
  //       VerboseError {
  //           errors: [
  //               (
  //                   "1\"hello\" : \"world\"\n  }\n  } ",
  //                   Char(
  //                       '}',
  //                   ),
  //               ),
  //               (
  //                   "{ 1\"hello\" : \"world\"\n  }\n  } ",
  //                   Context(
  //                       "map",
  //                   ),
  //               ),
  //               (
  //                   "{ \"a\"\t: 42,\n  \"b\": [ \"x\", \"y\", 12 ] ,\n  \"c\": { 1\"hello\" : \"world\"\n  }\n  } ",
  //                   Context(
  //                       "map",
  //                   ),
  //               ),
  //           ],
  //       },
  //   ),
  // )
  println!("parsed verbose: {:#?}", root::<VerboseError<&str>>(data));

  match root::<VerboseError<&str>>(data) {
    Err(Err::Error(e)) | Err(Err::Failure(e)) => {

      // here we use the `convert_error` function, to transform a `VerboseError<&str>`
      // into a printable trace.
      //
      // This will print:
      // verbose errors - `root::<VerboseError>(data)`:
      // 0: at line 2:
      //   "c": { 1"hello" : "world"
      //          ^
      // expected '}', found 1
      //
      // 1: at line 2, in map:
      //   "c": { 1"hello" : "world"
      //        ^
      //
      // 2: at line 0, in map:
      //   { "a" : 42,
      //   ^
      println!("verbose errors - `root::<VerboseError>(data)`:\n{}", convert_error(data, e));
    }
    _ => {},
  }
}