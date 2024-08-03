# build release
build:
    cargo build -r

# build debug
build-d:
    cargo build

# build specific
build-s package="sproc" database="sqlite":
    cargo build -r --package {{package}} --no-default-features --features {{database}}

build-s-d package="sproc" database="sqlite":
    cargo build --package {{package}} --no-default-features --features {{database}}

# run specific
run-s package="xsu-cliff" database="sqlite":
    cargo run --package {{package}} --no-default-features --features {{database}}

# run binary
run-b package:
    cargo run --package {{package}}

# ...
doc:
    cargo doc --no-deps --document-private-items
