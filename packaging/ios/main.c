// The whole iOS entry point: the game is Rust, and UIKit is started from inside it.
extern void urban_sprawl_ios_main(void);

int main(void) {
    urban_sprawl_ios_main();
    return 0;
}
