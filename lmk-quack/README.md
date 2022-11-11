# Quack
A lazy ducky script interpreter that implements a subset of [ducky script](https://docs.hak5.org/hak5-usb-rubber-ducky/ducky-script-quick-reference). Meaning the interpreter will continue on errors and do it's best to execute any script given to it. It will do very little work to verify the correctness of the scripts it's running.

## Examples

### Banner
```
STRINGLN      _      _      _      USB       _      _      _
STRINGLN   __(.)< __(.)> __(.)=   Rubber   >(.)__ <(.)__ =(.)__
STRINGLN   \___)  \___)  \___)    Ducky!    (___/  (___/  (___/
```

### Hello World
```
REM Types "Hello.....World!"

FUNCTION COUNTDOWN()
    WHILE ($TIMER > 0)
        STRING .
        $TIMER = ($TIMER - 1)
        DELAY 500
    END_WHILE
END_FUNCTION

STRING Hello
VAR $TIMER = 5
COUNTDOWN()
STRING World!
```

### Factorial
```
VAR $x = 0
FUNCTION FACT()
    VAR $i = 1
    VAR $total = 1
    WHILE ($i <= $x)
        $total = $total * $i
        $i = $i + 1
    END_WHILE
    STRINGLN $total
END_FUNCTION


$x = 10
STRING The factorial of 10 is 
FACT()
```

## Implemented
|Functionality|Status|Notes|
|-------------|------|-----|
|REM|✔️||
|STRINGLN|✔️||
|STRING|✔️||
|Cursor Keys|✔️||
|System Keys|✔️||
|Basic Modifier Keys|✔️||
|Advanced Modifier Keys|✔️||
|Standalone Modifier Keys|✔️|```INJECT_MOD``` not required|
|Lock Keys|✔️||
|Delays|✔️||
|The Button|||
|The LED|||
|Attack Mode||Cannot be implemented|
|Constants|✔️||
|Variables|✔️||
|Operators|✔️||
|Conditional Statements|✔️||
|Loops|✔️||
|Functions|✔️||
|Randomization|✔️|Except attack mode|
|Holding Keys|✔️||
|Payload Control|||
|Jitter|||
|Payload Hiding|||
|Wait For|||
|Save & Restore|||
|Exfiltration|||
|Internal Variables|||



## Usage
```
Usage: quack [OPTIONS] <INPUT>

Arguments:
  <INPUT>  Input script

Options:
  -s, --strict       Halt on errors
  -c, --no-comments  Hide comments
  -e, --no-errors    Hide errors
  -h, --help         Print help information
  -V, --version      Print version information
```