use clap::Parser;
use libwaysip::SelectionType;

use crate::cli::Cli;
use crate::settings::*;

fn cli(args: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("waysip").chain(args.iter().copied())).unwrap()
}

#[test]
fn dispatch_point() {
    let args = cli(&["-p"]);
    assert!(matches!(
        SelectionDispatch::from_cli(&args),
        SelectionDispatch::Point
    ));
}

#[test]
fn dispatch_dimensions_and_output() {
    let args = cli(&["-d", "-o"]);
    assert!(matches!(
        SelectionDispatch::from_cli(&args),
        SelectionDispatch::DimensionsOrOutput
    ));
}

#[test]
fn dispatch_dimensions_only() {
    let args = cli(&["-d"]);
    assert!(matches!(
        SelectionDispatch::from_cli(&args),
        SelectionDispatch::Area
    ));
}

#[test]
fn dispatch_output_only() {
    let args = cli(&["-o"]);
    assert!(matches!(
        SelectionDispatch::from_cli(&args),
        SelectionDispatch::Screen
    ));
}

#[test]
fn dispatch_screen_flag() {
    let args = cli(&["-i"]);
    assert!(matches!(
        SelectionDispatch::from_cli(&args),
        SelectionDispatch::Screen
    ));
}

/// Bare `waysip` (and `waysip -a 16:9`, `waysip -e`, `waysip -f ...`) used
/// to do nothing and exit 0; it now selects an area, like `-d` and slurp.
#[test]
fn dispatch_area_by_default() {
    for args in [&[][..], &["-a", "16:9"], &["-e"], &["-f", "%x\n"]] {
        assert!(
            matches!(
                SelectionDispatch::from_cli(&cli(args)),
                SelectionDispatch::Area
            ),
            "{args:?}"
        );
    }
}

/// `-r` is handled in `main` before the dispatch, so it doesn't pick a mode.
#[test]
fn dispatch_ignores_boxes_flag() {
    let args = cli(&["-r"]);
    assert!(matches!(
        SelectionDispatch::from_cli(&args),
        SelectionDispatch::Area
    ));
}

#[test]
fn selection_type_mapping() {
    assert!(matches!(
        SelectionDispatch::Point.selection_type(),
        SelectionType::Point
    ));
    assert!(matches!(
        SelectionDispatch::DimensionsOrOutput.selection_type(),
        SelectionType::DimensionsOrOutput
    ));
    assert!(matches!(
        SelectionDispatch::Area.selection_type(),
        SelectionType::Area
    ));
    assert!(matches!(
        SelectionDispatch::Screen.selection_type(),
        SelectionType::Screen
    ));
}

#[test]
fn resolve_output_format_uses_screen_template() {
    let mut args = cli(&["-i", "-f", "custom"]);
    let fmt = resolve_output_format(&mut args);
    assert!(fmt.starts_with("Screen : %o %d"));
}

#[test]
fn resolve_output_format_takes_custom_format_and_clears_it() {
    let mut args = cli(&["-f", "custom"]);
    let fmt = resolve_output_format(&mut args);
    assert_eq!(fmt, "custom");
    assert_eq!(args.format, "");
}

#[test]
fn resolve_output_format_default_when_unset() {
    let mut args = cli(&[]);
    let fmt = resolve_output_format(&mut args);
    assert_eq!(fmt, "%x,%y %wx%h\n");
}

#[test]
fn parse_hex_color_valid() {
    let color = parse_hex_color("#000000ff".to_string());
    assert_eq!((color.r, color.g, color.b, color.a), (0.0, 0.0, 0.0, 1.0));
}

#[test]
fn parse_aspect_ratio_valid() {
    assert_eq!(parse_aspect_ratio("16:9".to_string()), (16.0, 9.0));
}

/// `-c` (border and text) and `-s` (selection) used to set each other's
/// color: `-c 00ff00ff` filled the selection green.
#[test]
fn border_and_selection_colors_go_to_the_right_style_fields() {
    let mut args = cli(&["-c", "00ff00ff", "-s", "0000ffff"]);
    let builder = format!(
        "{:?}",
        builder_from_args(&mut args, SelectionType::Area, None)
    );
    let green = "Color { r: 0.0, g: 1.0, b: 0.0, a: 1.0 }";
    let blue = "Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 }";
    assert!(
        builder.contains(&format!("border_text_color: {green}")),
        "{builder}"
    );
    assert!(
        builder.contains(&format!("foreground_color: {blue}")),
        "{builder}"
    );
}
