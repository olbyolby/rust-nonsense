

pub trait Parser: Copy {
    type Target;
    type Location: Copy;

    fn current(&self) -> Option<Self::Target>;
    fn advance(&mut self); 
    
    fn location(self) -> Self::Location;
    
    fn consume(&mut self) -> Option<Self::Target> {
        let current = self.current();
        self.advance();

        current
    }
    fn advanced(&self) -> Self {
        let mut copy = *self;
        copy.advance();

        copy
    }
    
}

#[derive(Debug, Clone, Copy)]
pub struct Span<Location> {
    pub start: Location,
    pub end: Location
}
impl<Location> Span<Location> {
    pub fn new(start: Location, end: Location) -> Self {
        Self { start, end }
    }
}



#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StringLocation {
    pub byte: usize,
    pub character: usize
}
impl StringLocation {
    fn new(byte: usize, character: usize) -> Self {
        Self { byte, character }
    }
}

#[derive(Clone, Copy)]
pub struct StringParser<'a> {
    input: &'a str,
    
    index: usize,
    character: usize
}
impl<'a> StringParser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,

            index: 0,
            character: 0
        }
    }

    pub fn rest(&self) -> &str {
        &self.input[self.index..]
    }
}
impl<'a> Parser for StringParser<'a> {
    type Location = StringLocation;
    type Target = char;

    fn current(&self) -> Option<Self::Target> {
        self.input.get(self.index..)?.chars().next()
    }
    fn advance(&mut self) {
        if let Some(character) = self.current() {
            self.index += character.len_utf8();
            self.character += 1;
        }
    }

    fn location(self) -> Self::Location {
        StringLocation::new(self.index, self.character)
    }
}

#[derive(Clone, Copy)]
pub struct SliceParser<T> {
    data: T,

    index: usize
}
impl<'a, T> SliceParser<&'a [T]> {
    pub fn new(data: &'a [T]) -> Self {
        Self {
            data,

            index: 0
        }
    }
}
impl<'a, T> Parser for SliceParser<&'a [T]> {
    type Target =  &'a T;
    type Location = usize;

    fn current(&self) -> Option<Self::Target> {
        self.data.get(self.index)
    }
    fn advance(&mut self) {
        self.index += 1;
    }

    fn location(self) -> Self::Location {
        self.index
    }
}
