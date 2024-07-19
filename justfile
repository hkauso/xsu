build:
    cargo build -r

build-d:
    cargo build

publish:
    cargo publish --allow-dirty

doc:
    cargo doc --no-deps --document-private-items
