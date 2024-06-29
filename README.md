# sproc

**Sproc** is a simplified process manager that is designed to act as an interface for running **simple** services that aren't expected to exit.

## Config

Services are defined in Sproc using a simple `services.toml` file (passed into Sproc using the `pin` command).

```toml
[services.example]
command = "example --a b"           # required
working_directory = "/home/example" # required

  [services.example.environment]    # optional
  EXAMPLE_ENV_VAR = "42"
```

## Usage

Load config file:

```bash
sproc pin {path}
```

Start service:

```bash
sproc run {name}
```

Start all services:

```bash
sproc run-all
```

Stop a service:

```bash
sproc kill {name}
```

Stop all services:

```bash
sproc kill-all
```

Get running service info:

```bash
sproc info {name}
```

Get info about all running services:

```bash
sproc info-all
```
