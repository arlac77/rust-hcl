use hcl;

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
 }

