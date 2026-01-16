use tux_validation::os_release;

#[test]
fn debian_and_forky() {
    let osr = os_release::parse_os_release("/etc/os-release").expect("Failed to read os-release");

    assert_eq!(osr.get("ID").map(String::as_str), Some("debian"), "Not running Debian!");
    assert_eq!(osr.get("VERSION_CODENAME").map(String::as_str), Some("forky"), "Codename is not `forky`!");
}
