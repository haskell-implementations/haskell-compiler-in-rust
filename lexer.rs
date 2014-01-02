extern mod extra;
use extra::container::Deque;
use extra::ringbuf::RingBuf;
use std::iter::Peekable;
#[deriving(Eq, ToStr)]
enum TokenEnum {
	EOF,
	NAME,
	OPERATOR,
	NUMBER,
	FLOAT,
	LPARENS,
	RPARENS,
	LBRACKET,
	RBRACKET,
	LBRACE,
	RBRACE,
	INDENTSTART,
	INDENTLEVEL,
	COMMA,
	EQUALSSIGN,
	SEMICOLON,
	MODULE,
	CLASS,
	INSTANCE,
	WHERE,
	LET,
	IN,
	CASE,
	OF,
	ARROW,
	TYPEDECL,
	DATA
}

#[deriving(Eq, ToStr)]
struct Location {
    column : int,
    row : int,
    absolute : int
}

struct Token {
    token : TokenEnum,
    value : ~str,
    location : Location
}
impl Token {
    fn eof() -> Token {
        Token { token : EOF, value : ~"", location : Location { column : -1, row : -1, absolute : -1} }
    }

    fn new(token : TokenEnum, value : ~str, loc : Location) -> Token {
        Token { token : token, value : value, location : loc }
    }
    fn new_(token : TokenEnum, value : ~str) -> Token {
        Token::new(token, value, Location { column : -1, row : -1, absolute : -1 })
    }
}

impl Eq for Token {
    fn eq(&self, rhs : &Token) -> bool {
        self.token == rhs.token && self.value == rhs.value
    }
}

fn name_or_keyword(tok : &str) -> TokenEnum {
    match tok {
        "module" => MODULE,
        "class" => CLASS,
        "instance" => INSTANCE,
        "where" => WHERE,
        "let" => LET,
        "in" => IN,
        "case" => CASE,
        "of" => OF,
        "->" => ARROW,
        "data" => DATA,
        _ => NAME
    }
}

fn is_operator(first_char : char) -> bool {
    match first_char {
        '+' => true,
        '-' => true,
        '*' => true,
        '/' => true,
        '.' => true,
        '$' => true,
        ':' => true,
        '=' => true,
        '<' => true,
        '>' => true,
        '|' => true,
        '&' => true,
        _ => false
    }
}

struct Lexer<Stream> {
    input : Peekable<char, Stream>,
    location : Location,
    previousLocation : Location,
    unprocessedTokens : ~[Token],
    tokens : extra::ringbuf::RingBuf<Token>,
    indentLevels : ~[int],
    offset : uint
}


impl <Stream : Iterator<char>> Lexer<Stream> {
    
    pub fn new(input : Stream) -> Lexer<Stream> {
        let start = Location { column : 0, row : 0, absolute : 0};
        Lexer { 
            input : input.peekable(),
            location : start,
            previousLocation : start,
            unprocessedTokens : ~[],
            tokens : extra::ringbuf::RingBuf::with_capacity(20),
            indentLevels : ~[],
            offset : 0}
    }


    fn peek(&mut self) -> Option<char> {
        match self.input.peek() {
            Some(ch) => Some(*ch),
            None => None
        }
    }

    fn read_char(&mut self) -> Option<char> {
        match self.input.next() {
            Some(c) => {
                self.location = self.location;
                self.location.absolute += 1;
                self.location.column += 1;
                if (c == '\n' || c == '\r')
                {
                    self.location.column = 0;
                    self.location.row += 1;
                }
                Some(c)
            }
            None => None
        }
    }

    fn scan_digits(&mut self) -> ~str {
        let mut result = ~"";
        loop {
            match self.peek() {
                Some(x) => {
                    if !x.is_digit() {
                        return result;
                    }
                    self.read_char();
                    result.push_char(x)
                }
                None => return result
            }
        }
        result
    }

    fn scan_number(&mut self, c : char, location : Location) -> Token {
        let mut number = c.to_str() + self.scan_digits();
        let mut token = NUMBER;
        match self.peek() {
            Some('.') => {
                token = FLOAT;
                number.push_char('.');
                number = number.append(self.scan_digits());
            }
            _ => ()
        }
        Token { token : NUMBER, value : number, location : location }
    }

    pub fn next_<'a>(&'a mut self) -> &'a Token {
        self.next(|_| false)
    }
    pub fn next<'a>(&'a mut self, parseError : |TokenEnum| -> bool) -> &'a Token {
        if self.offset > 0 {
            match self.tokens.iter().idx(self.tokens.len() - 1 - self.offset) {
                Some(token) => token,
                None => fail!("Impossible empty tokens stream")
            }
        }
        else if self.unprocessedTokens.len() > 0 {
            self.layout_independent_token(parseError);
            self.tokens.back().unwrap()
        }
        else {
            self.new_token(parseError)
        }
    }
    
    fn new_token<'a>(&'a mut self, parseError : |TokenEnum| -> bool) -> &'a Token {
        let mut newline = false;
        let n = self.next_indent_token(&mut newline);
        self.unprocessedTokens.push(n);
        let newTok = self.unprocessedTokens[self.unprocessedTokens.len() - 1].token;

        if newTok != LBRACE {
            match self.tokens.back() {
                Some(tok) => {
                    if tok.token == LET || tok.token == WHERE || tok.token == OF {
                        let loc = self.unprocessedTokens[self.unprocessedTokens.len() - 1].location;
                        let indentstart = Token { token : INDENTSTART, value : ~"{n}", location : loc };
                        self.unprocessedTokens.push(indentstart);
                    }
                }
                None => ()
            }
        }
        if newline {
            self.unprocessedTokens.push(Token::new(INDENTLEVEL, ~"<n>", self.location));
        }
        self.layout_independent_token(parseError);
        self.tokens.back().unwrap()
    }

    fn layout_independent_token(&mut self, parseError : |TokenEnum| -> bool) {
        if self.unprocessedTokens.len() > 0 {
            let tok = self.unprocessedTokens[self.unprocessedTokens.len() - 1].token;
            let location = self.unprocessedTokens[self.unprocessedTokens.len() - 1].location;
            match tok {
                INDENTLEVEL => {
                    if (self.indentLevels.len() > 0) {
                        //m:ms
                        let m = self.indentLevels[self.indentLevels.len() - 1];
                        //m == n
                        if (m == location.column) {
                            self.tokens.push_back(Token::new(SEMICOLON, ~";", location));
                            self.unprocessedTokens.pop();
                            return;
                        }
                        else if (location.column < m)// n < m
                        {
                            //TODO
                            self.indentLevels.pop();
                            self.tokens.push_back(Token::new(RBRACE, ~"}", location));
                            return;
                        }
                    }
                    self.unprocessedTokens.pop();
                    if (self.unprocessedTokens.len() == 0) {
                        debug!("Skipping to next");
                        self.new_token(parseError);
                        return;
                    }
                    else {
                        return self.layout_independent_token(parseError);
                    }
                }
                INDENTSTART => {
                    //{n} token
                    let n = location.column;
                    if (self.indentLevels.len() != 0) {
                        //m:ms
                        let m = self.indentLevels[self.indentLevels.len() - 1];
                        if (n > m) {
                            self.unprocessedTokens.pop();
                            self.tokens.push_back(Token::new(LBRACE, ~"{", location));
                            self.indentLevels.push(n);
                            return;
                        }
                    }
                    if (n > 0)
                    {
                        self.tokens.push_back(Token::new(LBRACE, ~"{", location));
                        self.unprocessedTokens.pop();
                        self.indentLevels.push(n);
                        return;
                    }
                    self.tokens.push_back(Token::new(LBRACE, ~"{", location));
                    self.tokens.push_back(Token::new(RBRACE, ~"}", location));
                    self.unprocessedTokens.pop();
                    self.unprocessedTokens.push(Token::new(INDENTLEVEL, ~"<n>", location));
                    self.offset += 1;
                    return;
                }
                RBRACE => {
                    if (self.indentLevels.len() > 0 && self.indentLevels[self.indentLevels.len()] == 0) {
                        self.tokens.push_back(self.unprocessedTokens.pop());
                        self.indentLevels.pop();
                        return;
                    }
                    else {
                        return;//parse-error
                    }
                }
                LBRACE => {
                    self.tokens.push_back(self.unprocessedTokens.pop());
                    self.indentLevels.push(0);
                    return;
                }

                _ => ()
            }
            if (self.indentLevels.len() != 0) {
                //L (t:ts) (m:ms) 	= 	} : (L (t:ts) ms) 	if m /= 0 and parse-error(t)
                let m = self.indentLevels[self.indentLevels.len() - 1];
                if (m != 0 && parseError(tok))
                {
                    self.tokens.push_back(Token::new(RBRACE, ~"}", location));
                    self.indentLevels.pop();
                    return;
                }
            }
            self.tokens.push_back(self.unprocessedTokens.pop());
            return;
        }
        else {
            if (self.indentLevels.len() == 0)//End of stream
            {
                return;
            }
            else if (self.indentLevels[self.indentLevels.len() - 1] != 0)//Keep pusing righ brackets
            {
                self.indentLevels.pop();
                self.tokens.push_back(Token::new_(RBRACE, ~"}"));
                return;
            }
        }
    }

    fn next_indent_token(&mut self, newline : &mut bool) -> Token {
        let mut c = ' ';
        //Skip all whitespace before the token
        while c.is_whitespace() {
            match self.read_char() {
                Some(x) => {
                    c = x;
                    if (self.location.column == 0)//newline detected
                    {
                        *newline = true;
                    }
                }
                None => { return Token::eof() }
            }
        }
        let startLocation = self.location;

        //Decide how to tokenize depending on what the first char is
        //ie if its an operator then more operators will follow
        if (is_operator(c))
        {
            let mut result = c.to_str();
            loop {
                match self.peek() {
                    Some(ch) => {
                        if !is_operator(ch) {
                            break;
                        }
                        self.read_char();
                        result.push_char(ch);
                    }
                    None => { break; }
                }
            }
            let tok = match result {
                ~"="  => EQUALSSIGN,
                ~"->" => ARROW,
                ~"::" => TYPEDECL,
                _    => OPERATOR
            };
            return Token { token : tok, value : result, location : startLocation };
        }
        else if (c.is_digit())
        {
            return self.scan_number(c, startLocation);
        }
        else if (c.is_alphabetic() || c == '_')
        {
            let mut result = c.to_str();
            loop {
                match self.peek() {
                    Some(ch) => {
                        if !ch.is_alphanumeric() && ch != '_' {
                            break;
                        }
                        self.read_char();
                        result.push_char(ch);
                    }
                    None => return Token::eof()
                }
            }
            return Token {
                token : name_or_keyword(result),
                location : startLocation,
                value : result};
        }
        let tok = match c {
            ';' => SEMICOLON,
            '(' => LPARENS,
            ')' => RPARENS,
            '[' => LBRACKET,
            ']' => RBRACKET,
            '{' => LBRACE,
            '}' => RBRACE,
            ',' => COMMA,
            _   => EOF
        };
        Token { token : tok, location : startLocation, value : c.to_str() }
    }
}


#[test]
fn simple() {
    let mut lexer = Lexer::new("test 2 + 3".chars());

    assert_eq!(*lexer.next_(), Token::new_(NAME, ~"test"));
    assert_eq!(*lexer.next_(), Token::new_(NUMBER, ~"2"));
    assert_eq!(*lexer.next_(), Token::new_(OPERATOR, ~"+"));
    assert_eq!(*lexer.next_(), Token::new_(NUMBER, ~"3"));
}
#[test]
fn let_bind() {
    let mut lexer = Lexer::new(
r"let
    test = 2 + 3
in test".chars());

    assert_eq!(*lexer.next_(), Token::new_(LET, ~"let"));
    assert_eq!(*lexer.next_(), Token::new_(LBRACE, ~"{"));
    assert_eq!(*lexer.next_(), Token::new_(NAME, ~"test"));
    assert_eq!(*lexer.next_(), Token::new_(EQUALSSIGN, ~"="));
    assert_eq!(*lexer.next_(), Token::new_(NUMBER, ~"2"));
    assert_eq!(*lexer.next_(), Token::new_(OPERATOR, ~"+"));
    assert_eq!(*lexer.next_(), Token::new_(NUMBER, ~"3"));
}