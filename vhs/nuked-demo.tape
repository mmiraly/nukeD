Output assets/nuked-demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 760
Set Padding 18
Set Framerate 24
Set PlaybackSpeed 1.15
Set TypingSpeed 22ms
Set Theme {"name":"nukeD High Contrast","background":"#000000","foreground":"#F8F8F2","cursor":"#FFFFFF","selection":"#334155","black":"#000000","red":"#FF5C57","green":"#5AF78E","yellow":"#F3F99D","blue":"#57C7FF","magenta":"#FF6AC1","cyan":"#9AEDFE","white":"#F8F8F2","brightBlack":"#686868","brightRed":"#FF5C57","brightGreen":"#5AF78E","brightYellow":"#F3F99D","brightBlue":"#57C7FF","brightMagenta":"#FF6AC1","brightCyan":"#9AEDFE","brightWhite":"#FFFFFF"}

Hide
Type "cargo build --quiet"
Enter
Sleep 300ms

Type "export PATH=$PWD/target/debug:$PATH"
Enter
Sleep 100ms

Type "DEMO_ROOT=$(scripts/demo_fixture.sh)"
Enter
Sleep 200ms

Type "clear"
Enter
Sleep 200ms
Show

Type "nuked --root $DEMO_ROOT --dry-run --older-than 30d"
Enter
Sleep 2s

Type "nuked --root $DEMO_ROOT --older-than 30d"
Enter
Sleep 700ms

Type "1"
Sleep 450ms
Type "r"
Sleep 700ms
Type "h"
Sleep 500ms
Type "j"
Sleep 220ms
Enter
Sleep 350ms
Type "j"
Sleep 220ms
Enter
Sleep 350ms
Type " "
Sleep 450ms
Type "l"
Sleep 500ms
Type "/"
Sleep 220ms
Type "api"
Sleep 500ms
Enter
Sleep 400ms
Type " "
Sleep 450ms
Enter
Sleep 650ms
Enter
Sleep 1.4s
Type "?"
Sleep 1.4s
Type "q"
Sleep 400ms
