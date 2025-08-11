fn main() {
    cxx_build::bridge("src/lib.rs")
        .cpp(true)
        .std("c++14")
        .compile("rs_dfu");
}
