Output assets/nuked-demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1200
Set Height 760
Set Padding 18
Set Framerate 24
Set TypingSpeed 35ms
Set Theme "Catppuccin Mocha"

Type "cargo build --quiet"
Enter
Sleep 1s

Type "DEMO_ROOT=$(scripts/demo_fixture.sh)"
Enter
Sleep 400ms

Type "target/debug/nuked --root $DEMO_ROOT --dry-run --older-than 30d"
Enter
Sleep 3s

Type "target/debug/nuked --root $DEMO_ROOT --older-than 30d"
Enter
Sleep 1s

Type "1"
Sleep 700ms
Type "r"
Sleep 1s
Type "h"
Sleep 700ms
Type "j"
Sleep 300ms
Enter
Sleep 500ms
Type "j"
Sleep 300ms
Enter
Sleep 500ms
Type " "
Sleep 700ms
Type "l"
Sleep 700ms
Type "/"
Sleep 300ms
Type "api"
Sleep 700ms
Enter
Sleep 600ms
Type " "
Sleep 700ms
Enter
Sleep 900ms
Enter
Sleep 2s
Type "?"
Sleep 2s
Type "q"
Sleep 600ms
