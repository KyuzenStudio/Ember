# EmberScript Grammar v0.1 (EBNF)

Status: grammar v0.1 untuk Milestone 4. Setiap perubahan grammar dicatat di sini.
Implementasi: `crates/ember-lang`. Parser berhenti di error pertama (tanpa recovery).

## Lexical

```
ident    := [A-Za-z_][A-Za-z0-9_]*
int      := [0-9]+
float    := [0-9]+ "." [0-9]+
string   := '"' (escape | char)* '"'     // satu baris, tanpa newline mentah
escape   := '\"' | '\\' | '\n' | '\t'
comment  := '//' sampai akhir baris
```

Keyword: `var fn if else while return true false`.
Operator: `+ - * / % == != < <= > >= && || ! = ( ) { } , : .`
Tanpa eksponen numerik. Tanpa komentar blok. Angka butuh digit di kedua sisi titik.

## Syntax

```
program    := item*
item       := global_var | function
global_var := "var" ident ":" type "=" expr
function   := "fn" ident "(" [param ("," param)*] ")" block
param      := ident ":" type
type       := ident
block      := "{" stmt* "}"
stmt       := var_decl | assign | if_stmt | while_stmt | return_stmt | expr_stmt
var_decl   := "var" ident ":" type "=" expr
assign     := path "=" expr
if_stmt    := "if" expr block ["else" (block | if_stmt)]
while_stmt := "while" expr block
return_stmt:= "return" [expr]
expr_stmt  := expr
expr       := or
or         := and ("||" and)*
and        := equality ("&&" equality)*
equality   := comparison (("==" | "!=") comparison)*
comparison := additive (("<=" | ">=" | "<" | ">") additive)*
additive   := multiplicative (("+" | "-") multiplicative)*
multiplicative := unary (("*" | "/" | "%") unary)*
unary      := ("-" | "!") unary | call
call       := primary ("(" [expr ("," expr)*] ")")*
primary    := int | float | string | "true" | "false" | path | "(" expr ")"
path       := ident ("." ident)*
```

## Keputusan

- Satu file `.em` = satu `ScriptComponent`. `var` level file = state instance.
- `var` wajib bertipe dan berinisialisasi. Tanpa pemisah statement; statement boleh berjajar.
- Nama bertitik (`input.down`, `transform.move`) = path host API, bukan akses objek.
- `Key.Right` = konstanta bawaan `i32` (dicek Milestone 5).
- `return` tanpa nilai = return void. `return <expr>` rakus ke baris berikut.
- Blok `entity` ditunda sampai parser dan semantik stabil.

## Batas v0.1

- Parser lapor error pertama saja. Recovery ditunda.
- Tanpa generics, closures, macros, async, reflection, modul, GC, dynamic typing.
- Tipe diperiksa Milestone 5; nama tak dikenal ditolak di sana, bukan di sini.
