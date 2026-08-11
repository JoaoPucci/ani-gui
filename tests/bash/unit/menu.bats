#!/usr/bin/env bats
#
# Tests for ani-cli's `menu` dispatch (5.0).
#
# The chooser is user-configurable: fzf and rofi receive the prompt
# plus the multi-select and extra flag groups, dmenu a fixed list
# length plus the extra flags, and any other value is invoked as a
# command with the prompt as its final argument. Each arm is proven by
# substituting a recorder for the program it would launch — what
# matters is which program the setting selects and the argument shape
# it receives, not what a real picker draws.

load '../helpers/loader'

setup() {
    source_ani_cli_lib
}

fzf() { printf 'fzf %s' "$*"; }
rofi() { printf 'rofi %s' "$*"; }
dmenu() { printf 'dmenu %s' "$*"; }
recorder() { printf 'recorder %s' "$*"; }

@test "menu: fzf receives the prompt and both flag groups" {
    menu_program=fzf
    run menu "Choose" "--multi" "--height=10"
    assert_output "fzf --reverse --cycle --prompt Choose --multi --height=10"
}

@test "menu: rofi receives the prompt and both flag groups" {
    menu_program=rofi
    run menu "Choose" "-multi-select" "-theme x"
    assert_output "rofi -sort -dmenu -i -p Choose -multi-select -theme x"
}

@test "menu: dmenu receives the prompt and only the extra flags" {
    menu_program=dmenu
    run menu "Choose" "--multi" "-fn mono"
    assert_output "dmenu -l 20 -p Choose -fn mono"
}

@test "menu: any other program runs with the prompt as final argument" {
    menu_program=recorder
    run menu "Choose" "--multi" "--flag"
    assert_output "recorder --flag Choose"
}
