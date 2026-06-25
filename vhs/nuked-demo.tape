Output assets/nuked-demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 760
Set Padding 18
Set Framerate 24
Set PlaybackSpeed 1.15
Set TypingSpeed 22ms
Set Theme {"name":"nukeD Dark","background":"#050807","foreground":"#E8EEE7","cursor":"#D8FFE8","selection":"#17352F","black":"#050807","red":"#FF6B6B","green":"#7DDE92","yellow":"#F5C86A","blue":"#62B4FF","magenta":"#E778B2","cyan":"#79D8CF","white":"#D6DED8","brightBlack":"#6F7D76","brightRed":"#FF8A80","brightGreen":"#A8F0B6","brightYellow":"#FFE08A","brightBlue":"#8CCBFF","brightMagenta":"#F59BCB","brightCyan":"#A6F4EA","brightWhite":"#FFFFFF"}

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
