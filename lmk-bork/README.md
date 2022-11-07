# Bork
A terse keyboard scripting language used for key and mouse automation and recording.

## Examples

### Hello World
```
Hello, World!
```

### Fibonacci
```
<+#fib;#x;
    x <= 1 ? 
        x < 1 ? 0 : 1
    :
        <!fib;x-1> + <!fib;x-2>
>

The fibonacci of 10 is <!fib;10>.
```

### Greating
```
Hello <|echo $USER>,\n
\n
How are you doing today?
```


## Syntax
Bork is made up of four basic building blocks, characters that are outputted directly as key strokes, escapes, expression, and tags.

Escapes allow reserved characters to be evaluated as keystrokes and provide shortcuts for commonly used tags and functions. Expressions do mathematical computation within tags and functions. Tags provide control flow and access to full keyboard and mouse functionality.

All characters out side of an escape, expression or tag are treated as keystrokes excluding non-space whitespace.
### Data Types
| Type | Example| Description |
|------|--------|-------------|
|Literals|```"abcd"```|All characters inside the ```"``` will be treated as keystrokes to be sent. Aka literal keystrokes. Can include newlines.|
|Integers|```10``` ```-10```| A signed 64 bit integer. Typically used for mouse configurations |
|Booleans|```0``` ```F``` ```T```| Integers are also be treated as booleans. If an integer is ```0``` than it is false, otherwise it is true. You can also use a ```T``` or ```F``` as a substitute for ```0``` and ```1```|
|Unit| ```_``` | Unit is the result of certain operators that return no value. When used in a expression any operations done on it will result in the value of the other operand. When treated as a keystroke they resolve to ```0```.|
### Expressions
Expressions are values combined with operators.

#### Values
|Value|Example|Description|
|-----|-------|-----------|
|Brackets|```(x+2) * (x+1)```|Brackets can be used to group expression|
|Integers|```2```|Any integers.|
|Booleans|```T*2```| True or False.|
|Ascii| ```@a```| The number value of an ascii character. Used ```@character```.|
|Variables|```x+2```| A variable defined by the set tag or a function parameter. Only integer variables can be used in expressions.|
|Call|```<!add;2;3>```| Calls to functions with integer return types can be used in expressions.|
|LED State|```<&1>```| LED state tags can be used in expression. They will return a boolean.|

#### Operators
| Operator | Description | Precedence |
|----------|-------------|------------|
|*|Multiple|1|
|/|Divide|1|
|%|Modulus|1|
|+|Plus|2|
|-|Minus|2|
|~|Bitwise not|3|
|&|Bitwise and|3|
|\||Bitwise or|3|
|<<|Bitwise shift left|3|
|>>|Bitwise shift right|3|
|=|Equals|4|
|!=|Does not equal|4|
|<|Less than|4|
|>|Greater than|4|
|<=|Less than or equal|4|
|>=|Greater than or equal|4|
|!|Logical not|5|
|&&|Logical and|5|
|\|\||Logical or|5|
|? : |Ternary if operator. Used ```condition ? expression if true : expression if false```|6|
|*? _|While operator, where ```_``` is either ```_```, unit, or another binary operator excluding set. Used ```condition ?* expression +```. Will loop when the condition is true and apply the result of the expression to the next with the given operator unless ```_``` is given then it will return unit.|6|
|$| Set operator. Used ```expression $ variable```. Sets the variable to the expression. Returns unit.|6|

### Escapes
|Escape|Description|
|------|-----------|
|```\n```|Newline/Enter|
|```\t```|Tab|
|```\b```|Backspace|
|```\l```|Left mouse click|
|```\r```|Right mouse click|
|```\m```|Middle mouse click|
|```\\```|```\```|
|```\"```|```"```|
|```\$ \```| Print variable. Usage ```\$variable name\```|
|```\@ \```| Print number as ascii. Usage ```\@expression\```|
|```\x##```| Send keycode where ```##``` is the keycode in hex|
|```\<```|```<``` |

### Tags
|Name| Tag | Example | Description |
|----|---------|---------|------------|
|Key|```<>```|```<backspace>``` ```<crtl;"x">```|Sends keystroke of the corresponding special/modifier key. Keys can be chained together with ```;```. Keys can be chained with literals.|
|Hold|```<->``` ```<_>```|```<_"x">``` ```<-backspace>```|Hold or release a key. Will accept both special/modifier keys and literals. Keys can be chained. If the tag begins with ```<-``` then the key is released. If the tag begins with ```<_``` then the key is held.|
|If|```<? ; ; >```| ```<? x > 0; "x is greater than zero" ;? x < 0; "x is less than zero"  ; "x is zero" >```|An if statement. The basic building block of the tag is ```? condition ; result if true```. The first one is required and can then be chained one after the other with a ```;```. If a condition is true it's contents will be evaluated and then the tag will be exited skipping the rest. When a condition is false it's contents are skipped and the next condition is checked. The last block can omit the condition resulting in it being evaluated if all other conditions are false. Operators and variables can be used in conditions. |
|Loop|```<* ; >```|```<*x<0; "x is less than zero" >```|A loop. Evaluates whats between the ```;``` and ```>``` while the condition between the ```*``` and ```;``` is true. Operators and variables can be used in the condition.|
|Set|```<= ; >```|```<=x;0>```|Set the value of a variable. Used like ```<= variable name ; value >```. A variable can be set to a number. Operators can be used.|
|Print|```<$ >```|```<$x>```|Print the value of a variable.|
|Expression|```<# >```|```<#10+3>```| Evaluates an expression. Operators and variables can be used.|
|Click Mouse|```<^ >```| ```<@1>```| Clicks the given mouse button. ```1``` is the left mouse button. ```2``` is the right mouse button. ```3``` is the middle mouse button|
|Move Mouse|```<% ; >```|```<%10;-10>```|Move the mouse. ```<%x;y>``` The tag takes the relative x and y movement of the mouse as parameters. The values are capped between -127 and 127. Operators and variables can be used.|
|Pipe|```<\| >```|```<\|ls /dev/>```| Run the given command and return its stdout. The command will be run with bash. Tags can be used to generate commands inside the pipe tag. Only the ```\<``` escape can be used inside the tag. Whitespace is not ignored.|
|Function|```<+ ; ; >```|```<+"hello;"name;"Hello, World!\nMy name is <$name>">``` ```<+#add;#x;#y;x+y>```| Defines a function. The first parameter is the name of the function. The second is a list of input variables. The third is the content of the function that is evaluated when it is called. Function definitions return nothing. Functions must be defined before they can be used. Functions are expressional meaning the return value of the function is what the contents of the function evaluate too. All functions must evaluate to something. The type of the function return and the type of each parameter must be defined by prefixing their names with either ```"``` (a literal) or ```#``` (an integer). If the return type if ```#``` then the body of the function must be an expression. If the return type is ```"``` then the body of the function must be tags.|
|Call Function|```<! ; >``` | ```<!hello;"ella">``` ```<!add;10;1>```| Calls a function with the given inputs. The first parameter is the name of the function being called. The second is a list of input parameters.|
|Literal|```" "```| ```"hello"```| A literal can be used as a tag.|
|LED State|```<& >```|```<&1>```|Gets an led state. ```0``` if off. ```1``` if on. This can also be used anywhere integers can be used. Kana = 5, Compose = 4, ScrollLock = 3, CapsLock = 2, NumLock = 1|
|Ascii|```<@ @a + 10>```| Convert and expression to ascii.|