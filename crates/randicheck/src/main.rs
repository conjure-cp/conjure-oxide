// A prototype haskell parser that can parse data types and functions with case statements
use std::collections::HashMap;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Parser, Query, QueryCursor, Tree};

// a haskell data type has a name and a list of constructors
#[derive(Debug)]
struct DataType {
    Name: String,
    Constructors: Box<[Constructor]>,
}

//a constructor for an adt is just a list of types
#[derive(Clone,Debug)]
struct Constructor {
    Types: Box<[Type]>,
}
// we only support bools and custom types for now (may have to add boolean with a T/F or placeholder later)
#[derive(Clone,Debug)]
enum Type {
    Bool,
    Custom(String),
}

// a function has a name, it's input type(s), and it's output type
#[derive(Debug)]
struct Function {
    Name: String,
    Inputs: Box<[Type]>,
    Output: Box<Type>,
    Body: Box<[FunctionBody]>,
}
//the body of a function is either a case statement which has what it is matching on and a pattern, or a boolean
#[derive(Clone,Debug)]
enum FunctionBody {
    Case(String,Pattern),
    Bool,
}
// a pattern has a list of types that it matches on and gives a boolean
#[derive(Clone,Debug)]
struct Pattern {
    True_On: Box<[Type]>,
    gives : Bool}

#[derive(Clone,Debug)]
enum Bool {
    True,
    False,
}

// the full haskell file is a list of data types and functions, where we only support custom types, bools, and functions with case statements
#[derive(Debug)]
struct Haskell {
    adt: Box<[DataType]>,
    fun: Box<[Function]>,
}

fn main() {
    // get the haskell file
    let code = include_str!("tst.hs");

    // initialise parser
    let mut parser = Parser::new();
    let language = tree_sitter_haskell::LANGUAGE;
    parser
        .set_language(&language.into())
        .expect("Error loading Haskell language");
    // query to get the body of a function, which is a case statement, note that there is a recursion issue here at the apply function component
    // as a function may be curried and we can't represent that in the current query language
    let query_function = r#"
    (function name:
                (variable) @fun
                patterns: 
                (patterns (_)) @input 
            match: (match expression: (case (_)@matchcase alternatives: (alternatives alternative: (alternative pattern: ([(variable)@test (apply function: (_)@test (_)@test)]) match: (match expression: (_)@mtch))+ ) ))
    )
    "#;

    // gets all the data types and their constructors in the format of [(name, constructor, [field])], so we would have (X, L, Bool), (X, R, [Bool, Bool]),
    // hypothetically we could make it to be (name, [constructor, [field]]), but currently that's a shortcoming of the parser and query language due to sibling fields I haven't solved yet
    let adt_query = r#"
        declarations: 
            (declarations 
                (data_type name:
                    (name)@nm
                constructors:
                    (data_constructors constructor: 
                        (data_constructor constructor:
                            (prefix name:
                                    (constructor)@con
                                field:
                                    (name)+ @q
                            )
                        )
                    )+
                )
            )
    "#;
    let adtquery = Query::new(&language.into(), adt_query).unwrap();

    // currying is done via recursive results, and as such not supported right now
    // addendum, it would be normal for function signatures and functions to be right next to each other, but merging this with functions causes a seg fault for some reason
    let query_type_signature = r#"
    (signature name:
        (variable) @funName
            type:
                (function parameter:
                            (name) @param
                            result: 
                            [
                            (name) @result
                            (function parameter:
                                (name) @param
                                result: (_)@result)
                            ]    
                )
    )
    "#;
    let queryTypeSig = Query::new(&language.into(), query_type_signature).unwrap();

    // for recursion in type signatures later
    let queryRecurseSig = Query::new(
        &language.into(),
        r#"(function parameter: (name) @param result: (_)@result)"#,
    )
    .unwrap();

    let queryfunction = Query::new(&language.into(), query_function).unwrap();

    // cursor to iterate through the results of a query
    let mut cursor = QueryCursor::new();

    // make file for printing graph files
    // this is a debugging thing, can convert every stage in parsing to an svg using graphviz, not recommended though as the files are huge even for small input.
    //let mut file = File::create("graph.dot").unwrap();
    //parser.print_dot_graphs(&file);

    let parse_tree: Tree = parser.parse(code, None).unwrap();

    let root_node = parse_tree.root_node();
    //println!("{}\n\n", root_node.to_sexp());
    
    // get all the data types and their constructors
    let decls = cursor.matches(&adtquery, root_node, code.as_bytes());
    let mut decl: Vec<&str> = vec![];
    let mut constructor = HashMap::new();
    let mut adt = "";
    
    //Probably not rust like, need to investigate a better implementation
    decls.for_each(|m| {
        for capture in m.captures {
            match capture.index {
                // capture index's are the @ in the query, so 0 is the name of the data type, 1 is the constructor, and 2 is the field
                0 => {

                     if !decl.is_empty(){
                        //if we have a new constructor, add it to the hashmap, and clear the constructor declaration vector
                        constructor.entry(adt).or_insert_with(&mut Vec::new).push(decl.clone());
                        decl.clear();
                    } 
                    adt = &code[capture.node.start_byte()..capture.node.end_byte()]},
                1 => decl.push((&code[capture.node.start_byte()..capture.node.end_byte()])),
                2 => decl.push((&code[capture.node.start_byte()..capture.node.end_byte()])),
                //TODO: replace with a proper error, although this should be unreachable as QueryMatch would throw an error before this case
                otherwise => panic!()
            }
            
        }});
        constructor.entry(adt).or_insert_with(&mut Vec::new).push(decl.clone());
        let mut types = vec![];
        let mut adts = vec![];
        let mut constructorBoxes = vec![];
        // convert the hashmap to a vector of data types and then add them to the adts vector
        for (name,declr) in constructor {
            for cons in declr {
                for con in cons{
                    match con {
                        "Bool" => types.push(Type::Bool),
                        otherwise => types.push(Type::Custom(con.to_string()))
                    }            
                    

            }                    
            let con = Constructor{Types: types.clone().into()};
            constructorBoxes.push(con);
            types.clear();
            
        }
            adts.push(DataType{Name: name.to_string(), Constructors: constructorBoxes.clone().into()})

        }

    let type_signature = cursor.matches(&queryTypeSig, root_node, code.as_bytes());
    let mut signature="";
    let mut params = vec![];
    let mut result: Option<Type> = None;
    // signatures are in the format of (name, ([params], result))
    let mut signatures: HashMap<&str, (Vec<Type>, Type)> = HashMap::new();
    type_signature.for_each(|m| {
        for capture in m.captures {
            //0 for the function name, 1 for the parameters, 2 for the result
            match capture.index{
                0 => {
                    //assuming well typed input, a function should always have parameters (not dealing with constants yet)
                    if !params.is_empty(){
                       signatures.entry(signature).or_insert((params.clone(), <Option<Type> as Clone>::clone(&result).unwrap()));
                       params.clear();
                       result = None;
                   } 
                   signature = &code[capture.node.start_byte()..capture.node.end_byte()]},
               1 => {
                let param = &code[capture.node.start_byte()..capture.node.end_byte()];
                match param {
                   "Bool" => params.push(Type::Bool),
                   otherwise => params.push(Type::Custom(param.to_string()))    
                }},
                2 => {
                    let res = &code[capture.node.start_byte()..capture.node.end_byte()];
                    match res {
                       "Bool" => result = Some(Type::Bool),
                       otherwise => result = Some(Type::Custom(res.to_string())),    
                    }},
               otherwise => panic!()

            }
        }
    });    
    signatures.entry(signature).or_insert((params.clone(), <Option<Type> as Clone>::clone(&result).unwrap()));

    
    let function = cursor.matches(&queryfunction, root_node, code.as_bytes());
    let mut name = "";
    let mut patterns = vec![];
    let mut matching_on = "";
    let mut input = "";
    let mut case = vec![];
    let mut inputs: Vec<String> = vec![];
    let mut output = Bool::False;
    let mut functionBoxes = vec![];
    let mut bodies = vec![];
    function.for_each(|m| {
        for capture in m.captures {
            match capture.index{
                0 => {
                    
                    if !patterns.is_empty(){
                        let (params,result) = signatures.get(name).unwrap().clone();
                        for pattern in patterns.clone() {
                            let body = FunctionBody ::Case(matching_on.to_owned(), pattern);
                            bodies.push(body);
                        }
                        let function = Function{ Name: name.to_owned(), Inputs: params.into(), Output: Box::new(result), Body: bodies.clone().into()};
                        functionBoxes.push(function);
                        patterns.clear();
                        inputs.clear();
                        matching_on = "";
                    }
                    name = &code[capture.node.start_byte()..capture.node.end_byte()];
                },
                1 => input = &code[capture.node.start_byte()..capture.node.end_byte()],
                2 => matching_on = &code[capture.node.start_byte()..capture.node.end_byte()],
                3 => {
                    let param = &code[capture.node.start_byte()..capture.node.end_byte()];
                    // hacky way to do this while I haven't done the recursive part.
                    let inputs = param.split_whitespace().collect::<Vec<&str>>();
                    for item in inputs.clone(){
                        case.push(item);
                    }},
                4 => {
                    match &code[capture.node.start_byte()..capture.node.end_byte()] {
                        "True" => output = Bool::True,
                        "False" => output = Bool::False,
                        otherwise => panic!()
                    }
                    let mut casetypes = vec![];
                    for item in case.clone(){
                        match item {
                            "Bool" => casetypes.push(Type::Bool),
                            otherwise => casetypes.push(Type::Custom(item.to_string()))
                        }
                    } 
                    patterns.push(Pattern{ True_On: casetypes.into(), gives: output.clone()}); 
                    inputs.clear();
                    case.clear();
                }
                otherwise => panic!()
            }
        }
        
    });
    if !patterns.is_empty(){
        let (params,result) = signatures.get(name).unwrap().clone();
        for pattern in patterns {
            let body = FunctionBody ::Case(matching_on.to_owned(), pattern);
            bodies.push(body);
        }}
        let function = Function{ Name: name.to_owned(), Inputs: params.into(), Output: Box::new(result.unwrap()), Body: bodies.clone().into()};
   
    functionBoxes.push(function);

let haskell = Haskell{adt: adts.into(), fun: functionBoxes.into()};
    println!("{:?}", haskell);

}

/* this would be the recursive function to get the signature of a function, but we're not doing it right now because rust recursion is painful and this isn't the most important thing to do right now
fn recurse_signature<'a>(node: &'a tree_sitter::Node<'a>,cursor : &'a mut tree_sitter::QueryCursor, queryTypeSig: &'a tree_sitter::Query, code: &'a str) -> Vec<tree_sitter::QueryCapture<'a>> {
    let mut captures = vec![];
    let type_recurse = cursor.matches(&queryTypeSig, *node, code.as_bytes());

    type_recurse.for_each(|m| {
        for capture in m.captures {
            //println!("Capture: {:?}", capture);
            if capture.index == 2{
                if capture.node.kind() == "name" {

                    captures.push(capture);} else {
                        captures.append(&mut recurse_signature(&capture.node, cursor, queryTypeSig, code))
                    }

            } else {
            captures.push(capture);}
        }});

    return captures;
}
*/
