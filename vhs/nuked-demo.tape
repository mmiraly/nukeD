Output assets/nuked-demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 760
Set Padding 18
Set Framerate 24
Set PlaybackSpeed 1.15
Set TypingSpeed 22ms
Set Theme "GruvboxDark"

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
