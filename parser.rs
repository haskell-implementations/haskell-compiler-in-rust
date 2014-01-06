use std::str::{from_chars};
use std::util::{swap};
use std::hashmap::HashMap;
use lexer::*;
use lexer::{Lexer, Token, TokenEnum,
    EOF, NAME, OPERATOR, NUMBER, FLOAT, LPARENS, RPARENS, LBRACKET, RBRACKET, LBRACE, RBRACE, INDENTSTART, INDENTLEVEL, COMMA, EQUALSSIGN, SEMICOLON, MODULE, CLASS, INSTANCE, WHERE, LET, IN, CASE, OF, ARROW, TYPEDECL, DATA
};
use typecheck::{Type, TypeVariable, TypeOperator, Expr, Identifier, Number, Apply, Lambda, Let, Typed, function_type, identifier, apply, number, lambda, let_};

mod lexer;

pub struct Module {
    name : ~str,
    bindings : ~[Binding],
    typeDeclarations : ~[TypeDeclaration],
    classes : ~[Class],
    instances : ~[Instance],
    dataDefinitions : ~[DataDefinition]
}

pub struct Class {
    name : ~str,
    declarations : ~[TypeDeclaration]
}

pub struct Instance {
    bindings : ~[Binding],
    typ : TypeOperator,
    classname : ~str
}

pub struct Binding {
    name : ~str,
    expression : Typed<Expr>,
    typeDecl : TypeDeclaration
}

pub struct Constructor {
    name : ~str,
    typ : Type,
    tag : int,
    arity : int
}

pub struct DataDefinition {
    constructors : ~[Constructor],
    typ : TypeOperator,
    parameters : HashMap<~str, Type>
}

#[deriving(Clone)]
pub struct TypeDeclaration {
    context : ~[TypeOperator],
    typ : Type,
    name : ~str
}

pub struct Alternative {
    pattern : Pattern,
    expression : Typed<Expr>
}

pub enum Pattern {
    NumberPattern(int),
    IdentifierPattern(~str),
    ConstructorPattern(~str, ~[Pattern])
}
struct Parser<Iter> {
    lexer : Lexer<Iter>,
}

impl <Iter : Iterator<char>> Parser<Iter> {

pub fn new(iterator : Iter) -> Parser<Iter> {
    Parser { lexer : Lexer::new(iterator) }
}

fn requireNext<'a>(&'a mut self, expected : TokenEnum) -> &'a Token {
	let tok = self.lexer.next_().token;
	if (tok != expected) {
		fail!(ParseError(&self.lexer, expected));
    }
	return self.lexer.current();
}

fn module(&mut self) -> Module {
	let lBracketOrModule = self.lexer.next_().token;//tokenizeModule??
	let modulename = match lBracketOrModule {
        MODULE => {
            let modulename = self.requireNext(NAME).value.clone();
            self.requireNext(WHERE);
            self.requireNext(LBRACE);
            modulename
	    }
        LBRACE => {
		    //No module declaration was found so default to Main
		    ~"Main"
	    }
        _ => fail!(ParseError(&self.lexer, LBRACE))
    };

    let mut classes = ~[];
    let mut bindings = ~[];
    let mut instances = ~[];
    let mut typeDeclarations = ~[];
    let mut dataDefinitions = ~[];
	loop {
		//Do a lookahead to see what the next top level binding is
		let token = self.lexer.next(toplevelError).token;
		if (token == NAME || token == LPARENS)
		{
            let mut equalOrType = self.lexer.next(bindingError).token;
            {
			    let mut numberOfLookaheads = 2;
                while (equalOrType != TYPEDECL
                    && equalOrType != EQUALSSIGN)
                {
                    equalOrType = self.lexer.next(bindingError).token;
                    numberOfLookaheads += 1;
                }
                for _ in range(0, numberOfLookaheads)
                {
                    self.lexer.backtrack();
                }
            }

			if (equalOrType == TYPEDECL)
			{
				let bind = self.typeDeclaration();
				typeDeclarations.push(bind);
			}
			else
			{
				let bind = self.binding();
				bindings.push(bind);
			}
		}
		else if (token == CLASS)
		{
			self.lexer.backtrack();
			classes.push(self.class());
		}
		else if (token == INSTANCE)
		{
			self.lexer.backtrack();
			instances.push(self.instance());
		}
		else if (token == DATA)
		{
			self.lexer.backtrack();
			dataDefinitions.push(self.dataDefinition());
		}
		else
		{
			break;
		}
		let semicolon = self.lexer.next(toplevelNewBindError);
	    if (semicolon.token != SEMICOLON) {
            break;
        }
    }

	let rBracket = self.lexer.current().token;
	if (rBracket != RBRACE)
	{
		fail!(ParseError(&self.lexer, RBRACE));
	}

	let eof = self.lexer.next_();
	if (eof.token != EOF)
	{
		fail!("Unexpected token after end of module, {:?}", eof.token);
	}

	for decl in typeDeclarations.mut_iter()
	{
		for bind in bindings.mut_iter()
		{
			if (decl.name == bind.name)
			{
				bind.typeDecl = (*decl).clone();
			}
		}
	}
    Module {
        name : modulename,
        bindings : bindings,
        typeDeclarations : typeDeclarations,
        classes : classes,
        instances : instances,
        dataDefinitions : dataDefinitions }
}

fn class(&mut self) -> Class {
	self.requireNext(CLASS);

	let classname = self.requireNext(NAME).value.clone();
	let typeVariableName = self.requireNext(NAME).value.clone();
    let typeVariable = TypeVariable { id : 100 };

	self.requireNext(WHERE);
	self.requireNext(LBRACE);
	let mut typeVariableMapping = HashMap::new();
	typeVariableMapping.insert(typeVariableName, typeVariable);
	let declarations = self.sepBy1(|this| this.typeDeclaration_(&mut typeVariableMapping), SEMICOLON);
	
	self.lexer.backtrack();
	self.requireNext(RBRACE);

	Class { name : classname, declarations : declarations }
}

fn instance(&mut self) -> Instance {
	self.requireNext(INSTANCE);

	let classname = self.requireNext(NAME).value.clone();
	
	let typ = match self.parse_type() {
        TypeOperator(op) => op,
        _ => fail!("Expected type operator")
    };

	self.requireNext(WHERE);
	self.requireNext(LBRACE);

	let mut bindings = self.sepBy1(|this| this.binding(), SEMICOLON);
	for bind in bindings.mut_iter()
	{
		bind.name = encodeBindingIdentifier(typ.name, bind.name);
	}

	self.lexer.backtrack();
	self.requireNext(RBRACE);
	Instance { typ : typ, classname : classname, bindings : bindings }
}

fn expression_(&mut self) -> Typed<Expr> {
    match self.expression() {
        Some(expr) => expr,
        None => fail!("Failed to parse expression at {:?}", "ASD")
    }
}

fn expression(&mut self) -> Option<Typed<Expr>> {
	let app = self.application();
	self.parseOperatorExpression(app, 0)
}


fn parseList(&mut self) -> Typed<Expr> {
	let mut expressions = ~[];
	loop {
		match self.expression() {
            Some(expr) => expressions.push(expr),
            None => break
        }
		let comma = &self.lexer.next_();
        if (comma.token != COMMA) {
            break;
        }
	}

	if (expressions.len() == 0)
	{
		return Typed::new(Identifier(~"[]"));
	}

	let mut application;
	{
		let mut arguments = ~[Typed::new(Number(0)), Typed::new(Number(0))];//Must be 2 in length
		swap(&mut arguments[0], &mut expressions[expressions.len() - 1]);
		expressions.pop();
		arguments[1] = Typed::new(Identifier(~"[]"));

		application = makeApplication(Typed::new(Identifier(~":")), arguments);
	}
	while (expressions.len() > 0)
	{
		let mut arguments = ~[Typed::new(Number(0)), Typed::new(Number(0))];//Must be 2 in length
		swap(&mut arguments[0], &mut expressions[expressions.len() - 1]);
		expressions.pop();
		arguments[1] = application;

		application = makeApplication(Typed::new(Identifier(~":")), arguments);
	}

	let maybeParens = self.lexer.current().token;
	if (maybeParens != RBRACKET)
	{
		fail!(ParseError(&self.lexer, RBRACKET));
	}
	else
	{
		return application;
	}
}

fn subExpression(&mut self, parseError : |&Token| -> bool) -> Option<Typed<Expr>> {
	let token = self.lexer.next(parseError).token;
	match token {
	    LPARENS =>
		{
			let expressions = self.sepBy1(|this| this.expression_(), COMMA);

			let maybeParens = self.lexer.current();

			if (maybeParens.token != RPARENS)
			{
				fail!(ParseError(&self.lexer, RPARENS));
			}
			if (expressions.len() == 1)
			{
				Some(expressions[0])
			}
			else
			{
				Some(newTuple(expressions))
			}
		}
	    LBRACKET => Some(self.parseList()),
	    LET =>
		{
			self.requireNext(LBRACE);

			let binds = self.sepBy1(|this| this.binding(), SEMICOLON);

			let rBracket = self.lexer.current().token;
			if (rBracket != RBRACE)
			{
				fail!(ParseError(&self.lexer, RBRACE));
			}
			let inToken = self.lexer.next(letExpressionEndError).token;
			if (inToken != IN) {
				fail!(ParseError(&self.lexer, IN));
            }
			match self.expression() {
                Some(e) => {
                    let mut x = ~[];
                    for Binding { name : n, expression : exp, typeDecl : _ } in binds.move_iter() {
                        x.push((n, ~exp));
                    }
                    Some(Typed::new(Let(x, ~e)))
                }
                None => None
            }
		}
        /*
	    CASE =>
		{
			let expr = self.expression();

			self.requireNext(OF);
			self.requireNext(LBRACE);

			let alts = self.sepBy1(&Parser::alternative, SEMICOLON);
			let rBrace = self.lexer.current();
			if (rBrace.token != RBRACE)
			{
				fail!(ParseError(&self.lexer, RBRACE));
			}
			return Typed::with_location(Case(expr, alts), token.location);
		}*/
        NAME => {
            let token = self.lexer.current();
            Some(Typed::with_location(Identifier(token.value.clone()), token.location))
        }
        NUMBER => {
            let token = self.lexer.current();
            println!("Number {:?}", token.value);
            Some(Typed::with_location(Number(from_str(token.value).unwrap()), token.location))
        }
	    //FLOAT => Typed::with_location(Rational(token.value.from_str()), token.location),
	    _ => {
		self.lexer.backtrack();
        None
        }
    }
}

fn alternative(&mut self) -> Alternative {
	let pat = self.pattern();

	self.requireNext(ARROW);

	Alternative { pattern : pat, expression : self.expression_() }
}

fn parseOperatorExpression(&mut self, inL : Option<Typed<Expr>>, minPrecedence : int) -> Option<Typed<Expr>> {
	let mut lhs = inL;
    self.lexer.next_();
	while (self.lexer.valid() && self.lexer.current().token == OPERATOR
		&& precedence(self.lexer.current().value) >= minPrecedence)
	{
		let op = (*self.lexer.current()).clone();
		let mut rhs = self.application();
		let nextOP = self.lexer.next_().token;
		while (self.lexer.valid() && nextOP == OPERATOR
			&& precedence(self.lexer.current().value) > precedence(op.value))
		{
			let lookaheadPrecedence = precedence(self.lexer.current().value);
			self.lexer.backtrack();
			rhs = self.parseOperatorExpression(rhs, lookaheadPrecedence);
			self.lexer.next_();
		}
		let mut name = Typed::with_location(Identifier(op.value.clone()), op.location);
		let loc = match &lhs {
            &Some(ref l) => l.location,
            &None => op.location
        };
        lhs = match (lhs, rhs) {
            (Some(lhs), Some(rhs)) => {
                let args = ~[lhs, rhs];
                Some(makeApplication(name, args))
            }
            (Some(lhs), None) => {
                let args = ~[lhs, Typed::with_location(Identifier(~"#"), loc)];
                let mut apply = makeApplication(name, args);
                apply.location = loc;
                let params = ~[~"#"];
                Some(makeLambda(params, apply))
            }
            (None, Some(rhs)) => {
                if (op.value == ~"-")
                {
                    match name.expr {
                        Identifier(ref mut n) => *n = ~"negate",
                        _ => fail!("WTF")
                    }
                    let args = ~[rhs];
                    let mut l = makeApplication(name, args);
                    Some(l)
                }
                else
                {
                    let args = ~[Typed::with_location(Identifier(~"#"), loc), rhs];
                    let mut apply = makeApplication(name, args);
                    apply.location = loc;
                    let params = ~[~"#"];
                    let mut l = makeLambda(params, apply);
                    Some(l)
                }
            }
            (None, None) => return None
        };
	}
	self.lexer.backtrack();
	lhs
}

fn application(&mut self) -> Option<Typed<Expr>> {
    let e = self.subExpression(|t| false);
	match e {
        Some(lhs) => {
            let mut expressions = ~[];
            loop {
                let expr = self.subExpression(applicationError);
                match expr {
                    Some(e) => expressions.push(e),
                    None => break
                }
            }
            if (expressions.len() > 0)
            {
                let loc = lhs.location;
                lhs = makeApplication(lhs, expressions);//, loc);
            }
            Some(lhs)
        }
        None => None
    }
}

fn constructor(&mut self, dataDef : &DataDefinition) -> Constructor {
	let nameToken = self.lexer.next_().value.clone();
	let mut arity = 0;
	let typ = self.constructorType(&mut arity, dataDef);
	self.lexer.backtrack();
	Constructor { name : nameToken, typ : typ, tag : 0, arity : arity }
}

fn binding(&mut self) -> Binding {
	//name1 = expr
	//or
	//name2 x y = expr
	let nameToken = self.lexer.next(errorIfNotNameOrLParens).token;
	let mut name = self.lexer.current().value.clone();
	if (nameToken == LPARENS)
	{
		//Parse a name within parentheses
		let functionName = self.lexer.next(errorIfNotNameOrOperator).token;
		if (functionName != NAME && functionName != OPERATOR)
		{
			fail!("Expected NAME or OPERATOR on left side of binding {:?}", self.lexer.current().token);
		}
		name = self.lexer.current().value.clone();
        self.requireNext(RPARENS);
		let rParens = self.lexer.next(errorIfNotRParens).token;
		if (rParens != RPARENS)
		{
			fail!(ParseError(&self.lexer, RPARENS));
		}
	}
	else if (nameToken != NAME)
	{
		fail!(ParseError(&self.lexer, NAME));
	}

	//Parse the arguments for the binding
	let mut arguments = ~[];
	while (true)
	{
		let token = self.lexer.next(errorIfNotNameOrEqual);
		if (token.token == NAME)
		{
			arguments.push(token.value.clone());
		}
		else
		{
			break;
		}
	}
	if (self.lexer.current().token != EQUALSSIGN)
	{
		fail!(ParseError(&self.lexer, EQUALSSIGN));
	}
	if (arguments.len() > 0)
    {
		let lambda = makeLambda(arguments, self.expression_());
		Binding { name : name, typeDecl : TypeDeclaration { context : ~[], typ : Type::new_var(-1), name : ~"" }, expression : lambda }
	}
	else
	{
		Binding { name : name, typeDecl : TypeDeclaration { context : ~[], typ : Type::new_var(-1), name : ~"" }, expression : self.expression_() }
	}
}


fn patternParameter(&mut self) -> ~[Pattern] {
	let mut parameters = ~[];
	loop {
		let token = self.lexer.next_().token;
		match token
		{
            NAME => parameters.push(IdentifierPattern(self.lexer.current().value.clone())),
            NUMBER => parameters.push(NumberPattern(from_str(self.lexer.current().value.clone()).unwrap())),
		    LPARENS =>
			{
				let pat = self.pattern();
				let maybeComma = self.lexer.next_().token;
				if (maybeComma == COMMA)
				{
					let mut tupleArgs = self.sepBy1(|this| this.pattern(), COMMA);

					let rParens = self.lexer.current();
					if (rParens.token != RPARENS)
					{
						fail!(ParseError(&self.lexer, RPARENS));
					}
					tupleArgs.unshift(pat);
					parameters.push(ConstructorPattern(tuple_name(tupleArgs.len()), tupleArgs));
				}
				else
				{
                    //TODO?
				}
			}
		    _ => { break; }
		}
	}
	self.lexer.backtrack();
	return parameters;
}

fn pattern(&mut self) -> Pattern {
	let nameToken = self.lexer.next_().token;
    let name = self.lexer.current().value.clone();
	match nameToken {
	    LBRACKET =>
		{
			if (self.lexer.next_().token != RBRACKET)
			{
				fail!(ParseError(&self.lexer, RBRACKET));
			}
			ConstructorPattern(~"[]", ~[])
		}
	    NAME | OPERATOR =>
		{
			let patterns = self.patternParameter();
			if (name.char_at(0).is_uppercase() || name == ~":")
			{
				ConstructorPattern(name, patterns)
			}
			else
			{
				assert!(patterns.len() == 0);
				IdentifierPattern(name)
			}
		}
	    NUMBER => NumberPattern(from_str(name).unwrap()),
	    LPARENS =>
		{
			let tupleArgs = self.sepBy1(|this| this.pattern(), COMMA);
			let rParens = self.lexer.current().token;
			if (rParens != RPARENS) {
				fail!(ParseError(&self.lexer, RPARENS));
			}
			ConstructorPattern(tuple_name(tupleArgs.len()), tupleArgs)
		}
	    _ => { fail!("Error parsing pattern") }
	}
}

fn typeDeclaration(&mut self) -> TypeDeclaration {
	let mut typeVariableMapping = HashMap::new();
	self.typeDeclaration_(&mut typeVariableMapping)
}

fn typeDeclaration_(&mut self, typeVariableMapping : &mut HashMap<~str, TypeVariable>) -> TypeDeclaration {
    let mut name;
	{
        let nameToken = self.lexer.next(errorIfNotNameOrLParens).token;
        name = self.lexer.current().value.clone();
        if (nameToken == LPARENS) {
            //Parse a name within parentheses
            let functionName = self.lexer.next(errorIfNotNameOrOperator).token;
            if (functionName != NAME && functionName != OPERATOR)
            {
                fail!("Expected NAME or OPERATOR on left side of binding {:?}", functionName);
            }
            name = self.lexer.current().value.clone();
            let rParens = self.lexer.next(errorIfNotRParens).token;
            if (rParens != RPARENS)
            {
                fail!(ParseError(&self.lexer, RPARENS));
            }
        }
        else if (nameToken != NAME) {
            fail!(ParseError(&self.lexer, NAME));
        }
    }
	let decl = self.lexer.next_().token;
	if (decl != TYPEDECL) {
		fail!(ParseError(&self.lexer, TYPEDECL));
	}
	let typeOrContext = self.parse_type_(typeVariableMapping);
    {
        let maybeContextArrow = self.lexer.next_().token;
        if (maybeContextArrow == OPERATOR && self.lexer.current().value == ~"=>") {
            let t = self.parse_type_(typeVariableMapping);
            let op = match typeOrContext {
                TypeOperator(x) => x,
                _ => fail!("Expected type context since '=>' was parsed")
            };
            return TypeDeclaration { name : name, typ : t, context : createTypeConstraints(op) };
        }
    }
	self.lexer.backtrack();
	TypeDeclaration { name : name, typ : typeOrContext, context : ~[] }
}

fn constructorType(&mut self, arity : &mut int, dataDef : &DataDefinition) -> Type
{
	let token = self.lexer.next(constructorError).token;
	if (token == NAME) {
		*arity += 1;
		if (self.lexer.current().value.char_at(0).is_lowercase())
		{
			match dataDef.parameters.find(&self.lexer.current().value) {
                Some(existingVariable) => { 
                    function_type(existingVariable, &self.constructorType(arity, dataDef))
                }
                None => fail!("Undefined type parameter {:?}", self.lexer.current().value)
            }
		}
		else {
			function_type(&Type::new_op(self.lexer.current().value.clone(), ~[]), &self.constructorType(arity, dataDef))
        }
	}
	else {
		TypeOperator(dataDef.typ.clone())
	}
}


fn dataDefinition(&mut self) -> DataDefinition {
	self.requireNext(DATA);
	let dataName = self.requireNext(NAME).value.clone();

	let mut definition = DataDefinition {
        constructors : ~[],
        typ : TypeOperator { name : dataName, types : ~[]},
        parameters : HashMap::new()
    };
	while (self.lexer.next_().token == NAME)
	{
		definition.typ.types.push(Type::new_var(-1));
        let typ = definition.typ.types[definition.typ.types.len() - 1].clone();
		definition.parameters.insert(self.lexer.current().value.clone(), typ);
	}
	let equalToken = self.lexer.current().token;
	if (equalToken != EQUALSSIGN)
	{
		fail!(ParseError(&self.lexer, EQUALSSIGN));
	}
	definition.constructors = self.sepBy1_func(|this| this.constructor(&definition),
		|t : &Token| t.token == OPERATOR && t.value == ~"|");
	for ii in range(0, definition.constructors.len())
	{
		definition.constructors[ii].tag = ii as int;
	}
	self.lexer.backtrack();
	definition
}


fn parse_type(&mut self) -> Type {
	let mut vars = HashMap::new();
	return self.parse_type_(&mut vars);
}

fn parse_type_(&mut self, typeVariableMapping : &mut HashMap<~str, TypeVariable>) -> Type {
	let result = Type::new_var(0);
	let token = (*self.lexer.next_()).clone();
	match token.token {
	    LBRACKET =>
		{
			let t = self.parse_type_(typeVariableMapping);
			self.requireNext(RBRACKET);
			let args = ~[t];
			let listType = Type::new_op(~"[]", args);
            
            return self.parse_return_type(listType, typeVariableMapping);
		}
	    LPARENS =>
		{
			let t = self.parse_type_(typeVariableMapping);
			let maybeComma = self.lexer.next_().token;
			if (maybeComma == COMMA)
			{
				let mut tupleArgs = self.sepBy1(|this| this.parse_type_(typeVariableMapping), COMMA);
				tupleArgs.unshift(t);
                self.requireNext(RPARENS);

                return self.parse_return_type(tupleType(tupleArgs), typeVariableMapping);
			}
			else if (maybeComma == RPARENS)
			{
                return self.parse_return_type(t, typeVariableMapping);
			}
		}
	    NAME =>
		{
			let mut typeArguments = ~[];
            {
                loop {
                    let next = &self.lexer.next_();
                    if next.token != NAME {
                        break;
                    }
                    let var = typeVariableMapping.find_or_insert(next.value.clone(), TypeVariable { id : -1});
                    typeArguments.push(TypeVariable(*var));
                }
            }
            let next : Token = (*self.lexer.current()).clone();
			let mut thisType = Type::new_var(0);
			if (token.value.char_at(0).is_uppercase())
			{
				thisType = Type::new_op(token.value, typeArguments);
			}
			else
			{
                let t = typeVariableMapping.find_or_insert(token.value, TypeVariable { id : -1});
				thisType = TypeVariable(t.clone());
			}
			return self.parse_return_type(thisType, typeVariableMapping);
		}
	    _ => { return Type::new_var(-1); }
	};
    return Type::new_var(-1);
}

fn parse_return_type(&mut self, typ : Type, typeVariableMapping : &mut HashMap<~str, TypeVariable>) -> Type {

    let arrow = self.lexer.next_().token;
    if (arrow == ARROW) {
        return function_type(&typ, &self.parse_type_(typeVariableMapping));
    }
    else {
        self.lexer.backtrack();
        return typ
    }
}

fn sepBy1<T>(&mut self, f : |&mut Parser<Iter>| -> T, sep : TokenEnum) -> ~[T] {
    self.sepBy1_func(f, |tok| tok.token == sep)
}

fn sepBy1_func<T>(&mut self, f : |&mut Parser<Iter>| -> T, sep : |&Token| -> bool) -> ~[T] {
    let mut result = ~[];
    loop {
        result.push(f(self));
        if (sep(self.lexer.next_())) {
            break;
        }
    }
    result
}
}//end impl Parser

fn precedence(s : &str) -> int {
    match s {
        "+" => 1,
        "-" => 1,
        "*" => 3,
        "/" => 3,
        "%" => 3,
        "==" => 1,
        "/=" => 1,
        "<" => 1,
        ">" => 1,
        "<=" => 1,
        ">=" => 1,
        _ => 9
    }
}


fn toplevelError(t : &Token) -> bool
{
	return t.token != NAME
		&& t.token != RBRACKET
		&& t.token != SEMICOLON
		&& t.token != DATA
		&& t.token != LPARENS
		&& t.token != CLASS
		&& t.token != INSTANCE;
}

fn toplevelNewBindError(t : &Token) -> bool
{
	return t.token != RBRACKET
		&& t.token != SEMICOLON;
}

fn bindingError(t : &Token) -> bool
{
	return t.token != EQUALSSIGN
		&& t.token != NAME
		&& t.token != TYPEDECL
		&& t.token != OPERATOR
		&& t.token != RPARENS;
}

fn constructorError(tok : &Token) -> bool
{
	return tok.token != NAME
		&& tok.token != OPERATOR
		&& tok.token != LPARENS;
}

fn tuple_name(size : uint) -> ~str
{
	let mut name = ~"(";
    for _ in range(1, size) {
        name.push_char(',');
    }
	name.push_char(')');
	name
}

fn makeApplication(f : Typed<Expr>, args : ~[Typed<Expr>]) -> Typed<Expr> {
	assert!(args.len() >= 1);
    let mut func = f;
	for a in args.move_iter() {
		func = Typed::new(Apply(~func, ~a));
	}
    func
}
fn makeLambda(a : ~[~str], body : Typed<Expr>) -> Typed<Expr> {
    let mut args = a;
	assert!(args.len() >= 1);
	let mut body = body;
    let mut ii = args.len() - 1;
	while ii >= 0 {
		body = Typed::new(Lambda(args.pop(), ~body));
        ii -= 1;
	}
    body
}

//Create a tuple with the constructor name inferred from the number of arguments passed in
fn newTuple(arguments : ~[Typed<Expr>]) -> Typed<Expr> {
	let name = Typed::new(Identifier(tuple_name(arguments.len())));
	makeApplication(name, arguments)
}

fn subExpressionError(t : &Token) -> bool {
	t.token != LPARENS
		&& t.token != LET
		&& t.token != CASE
		&& t.token != NAME
		&& t.token != NUMBER
		&& t.token != FLOAT
		&& t.token != SEMICOLON
		&& t.token != LBRACKET
}

fn letExpressionEndError(t : &Token) -> bool {
	t.token != IN
}

fn applicationError(t :&Token) -> bool
{
	return t.token != LPARENS
		&& t.token != RPARENS
		&& t.token != LBRACKET
		&& t.token != RBRACKET
		&& t.token != LET
		&& t.token != OF
		&& t.token != NAME
		&& t.token != NUMBER
		&& t.token != FLOAT
		&& t.token != OPERATOR
		&& t.token != SEMICOLON
		&& t.token != COMMA;
}


fn errorIfNotNameOrLParens(tok : &Token) -> bool {
    tok.token != NAME && tok.token != LPARENS
}
fn errorIfNotIdentifier(tok : &Token) -> bool {
	tok.token != NAME
}
fn errorIfNotNameOrOperator(tok : &Token) -> bool {
	tok.token != NAME && tok.token != OPERATOR
}

fn errorIfNotNameOrEqual(tok : &Token) -> bool {
	tok.token != NAME && tok.token != EQUALSSIGN
}
fn errorIfNotRParens(tok : &Token) -> bool {
	tok.token != RPARENS
}

fn createTypeConstraints(context : TypeOperator) -> ~[TypeOperator] {
	let mut mapping = ~[];

	if (context.name.char_at(0) == '(') {
		for t in context.types.move_iter() {
            let op = match t {
                TypeOperator(op) => op,
                _ => fail!("Expected TypeOperator when creating constraints")
            };
			mapping.push(op);
		}
	}
	else {
		mapping.push(context.clone());
	}
	mapping
}

fn typeParseError(t : &Token) -> bool
{
	return t.token != ARROW
		&& t.token != SEMICOLON
		&& t.token != RBRACE
		&& t.token != RPARENS
		&& t.token != RBRACKET;
}

fn tupleType(types : ~[Type]) -> Type {
	Type::new_op(tuple_name(types.len()), types)
}

fn ParseError<Iter>(lexer : &Lexer<Iter>, expected : TokenEnum) -> ~str {
    format!("Expected {:?}", expected)
}
fn encodeBindingIdentifier(instancename : &str, bindingname : &str) -> ~str {
    fail!("Unimplemented function encodeBinding " + instancename + " " + bindingname);
}


#[test]
fn simple()
{
    let mut parser = Parser::new("2 + 3".chars());
    let expr = parser.expression_();
    assert_eq!(expr, apply(apply(identifier(~"+"), number(2)), number(3)));
}