# build release
build:
    cargo build -r

# build debug
build-d:
    cargo build

# build specific
build-s package="sproc" database="sqlite":
    cargo build --package {{package}} --no-default-features --features {{database}}

# ...
doc:
    cargo doc --no-deps --document-private-items
