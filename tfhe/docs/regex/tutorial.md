# FHE Regex Pattern Matching Tutorial

This tutorial explains what went into building the example Regex pattern matching engine.

Regexes are

There are two main components identifiable in a pattern matching engine (PME):
1. the pattern that is to be matched has to be parsed, this translates from a textual representation into a recursively structured object (an Abstract Syntax Tree, or AST)
2. this AST has to then be applied to the text that is to be matched against, resulting in a yes or no to whether the pattern matched (and in the case of our FHE implementation, this result is an encrypted yes or an encrypted no)

Parsing is a well understood problem. There are a couple of different approaches possible here.
Regardless of the approach chosen, it starts with figuring out what the language is that we want to support.
There actually exists a language that can help us describe exactly what our own language's structure is: Grammars.

## The Grammar and Datastructure

A Grammar consists of (generally a small) set of rules. For example, a very basic Grammar could look like this:
```
Start := 'a'
```
this describes a language that only contains the sentence "a". Not a very interesting language.

We can make it more interesting though by introducing choice into the grammar with \| operators. If we want the above grammar to accept either "a" or "b":
```
Start := 'a' | 'b'
```

So far only Grammars with a single start rule have been shown. However, a Grammar can consist of multiple rules.
And in fact, most languages require to be defined over multiple rules.
Lets consider a more meaningful language, one that accepts sentences consisting of one or more digits, we could describe such a language with the following Grammar:
```
Start := Digit+

Digit := '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9'
```

In the case of the example PME the grammar is as follows (apologies for the mixture of special tokens between the Grammar language and the pattern language, notice the quoted variants ...)
```
Start := '/' '^'? Regex '$'? '/'

Regex := Term '|' Term
       | Term

Term := Factor*

Factor := Atom '?'
        | Repeated
        | Atom

Repeated := Atom '*'
          | Atom '+'
          | Atom '{' Digit* ','? '}'
          | Atom '{' Digit+ ',' Digit* '}'

Atom := '.'
      | '\' .
      | Character
      | '[' Range ']'
      | '(' Regex ')'

Range := '^' Range
       | Letter '-' Letter
       | Letter+

Digit := '0' .. '9'

Character := Letter
           | ..

Letter := 'a' .. 'z'
        | 'A' .. 'Z'
```

With the Grammar defined, we can start defining a type to parse into. In Rust we
have the `enum` kind of type that is perfect for this, as it allows to define
multiple variants that may recurse. I prefer to start by defining variants that
do not recurse (ie that don't contain nested regex expressions):
```rust
enum RegExpr {
    Char { c: char },  // matching against a single character (Atom.2 and Atom.3)
    AnyChar,  // matching _any_ character (Atom.1)
    SOF,  // matching only at the beginning of the content ('^' in Start.1)
    EOF,  // matching only at the end of the content (the '$' in Start.1)
    Range { cs: Vec<char> },  // matching on a list of characters (Range.3, eg '[acd]')
    Between { from: char, to: char },  // matching between 2 characters based on ascii ordering (Range.2, eg '[a-g]')
}
```

With this we would already be able to translate the following basic regexes:

Pattern | RegExpr value
--- | ---
`/a/` | `RegExpr::Char { c: 'a' }`
`/\\^/` | `RegExpr::Char { c: '^' }`
`/./` | `RegExpr::AnyChar`
`/^/` | `RegExpr::SOF`
`/$/` | `RegExpr::EOF`
`/[acd]/` | `RegExpr::Range { vec!['a', 'c', 'd'] }`
`/[a-g]/` | `RegExpr::Between { from: 'a', to: 'g' }`

Notice we're not yet able to sequence multiple components together. Lets define
the first variant that captures recursive RegExpr for this:
```rust
enum RegExpr {
    ...
    Seq { re_xs: Vec<RegExpr> },  // variant for matching sequences of RegExpr components (Term.1)
}
```
With this Seq (short for sequence) variant we allow translating patterns that contain multiple components:

Pattern | RegExpr value
--- | ---
`/ab/` | `RegExpr::Seq { re_xs: vec![RegExpr::Char { c: 'a' }, RegExpr::Char { c: 'b' }] }`
`/^a.$/` | `RegExpr::Seq { re_xs: vec![RegExpr::SOF, RexExpr::Char { 'a' }, RegExpr::AnyChar, RegExpr::EOF] }`
`/a[f-l]/` | `RegExpr::Seq { re_xs: vec![RegExpr::Char { c: 'a' }, RegExpr::Between { from: 'f', to: 'l' }] }`

Lets finish the RegExpr datastructure by adding for optional matching, the not logic in a range, and the either left or right matching:
```rust
enum RegExpr {
    ...
    Optional { opt_re: Box<RegExpr> },  // matching optionally (Factor.1)
    Not { not_re: Box<RegExpr> },  // matching inversely on a range (Range.1)
    Either { l_re: Box<RegExpr>, r_re: Box<RegExpr> },  // matching the left or right regex (Regex.1)
}
```

We are now able to translate any complex regex into a RegExpr value. For example:

Pattern | RegExpr value
--- | ---
`/a?/` | `RegExpr::Optional { opt_re: Box::new(RegExpr::Char { c: 'a' }) }`
`/[a-d]?/` | `RegExpr::Optional { opt_re: Box::new(RegExpr::Between { from: 'a', to: 'd' }) }`
`/[^ab]/` | `RegExpr::Not { not_re: Box::new(RegExpr::Range { cs: vec!['a', 'b'] }) }`
`/av\|d?/` | `RegExpr::Either { l_re: Box::new(RegExpr::Seq { re_xs: vec![RegExpr::Char { c: 'a' }, RegExpr::Char { c: 'v' }] }), r_re: Box::new(RegExpr::Optional { opt_re: Box::new(RegExpr::Char { c: 'd' }) }) }`
`/(av\|d)?/` | `RegExpr::Optional { opt_re: Box::new(RegExpr::Either { l_re: Box::new(RegExpr::Seq { re_xs: vec![RegExpr::Char { c: 'a' }, RegExpr::Char { c: 'v' }] }), r_re: Box::new(RegExpr::Char { c: 'd' }) }) }`

With the Grammar defined, and the datastructure to parse into defined, we can now start implementing the actual parsing logic. There are many ways this can be done. For example there exist tools that can automatically generate code by giving it the Grammar definition (these are called parser generators). However, I prefer to write parsers myself with a parser combinator library, as in my opinion the behavior in runtime is better understandable of these than of parsers that were automatically generated.

In Rust there exist a number of popular parser combinator libraries, I went with `combine` but any other would work just as well. Choose whichever appeals the most to you. The implementation of our regex parser will differ significantly depending on the approach you choose and as such I think it is better to omit this part from the tutorial. You may look at the parser code in the example implementation to get an idea on how this could be done.



Any pattern matching engine has to

Building a Regex engine for matching against

# How to apply the example implementation in your own code

First include the relevant dependencies:

```rust
use tfhe::regex::ciphertext::{gen_keys, encrypt_str};
use tfhe::regex::engine::has_match;
```

Then, generate a private and public key pair:

```rust
let (client_key, server_key) = gen_keys();
```

Encrypt the content, this generates a `StringCiphertext` from a `&str`. The
content can only contain ascii characters, if there are any non-ascii symbols
present `encrypt_str` below will throw an error:

```rust
let ct_content = encrypt_str(&client_key, 'some body of text')?;
```

Apply your regex pattern to the generated ciphertext content:

```rust
let ct_res = has_match(&server_key, &ct_content, '/^ab|cd$/')?;
```

The result (`ct_res` here) is an encrypted ciphertext and must therefore first
be decrypted with the client key:

```rust
let res: u64 = client_key.decrypt(&ct_res);
```
once decrypted (`res` here), it will be either `0` for no match or `1` for a
match.


## Complete example

Here are above steps in a single code block:

```rust
use tfhe::regex::ciphertext::{gen_keys, encrypt_str};
use tfhe::regex::engine::has_match;

fn main(content: &str, pattern: &str) {
    let content = "Content that will be encrypted and pattern matched against";
    let pattern = "/w(i|a)ll/";

    println!("generating the keys..");
    let (client_key, server_key) = gen_keys();

    println!("encrypting content..");
    let ct_content = encrypt_str(&client_key, content);

    println!("applying regex..");
    let ct_res = has_match(&server_key, &ct_content.unwrap(), pattern).unwrap();
    let res: u64 = client_key.decrypt(&ct_res);
    println!("res: {:?}", res);
}
```
