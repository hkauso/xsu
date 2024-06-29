# sproc

**Sproc** is a simplified process manager that uses a composable configuration file to manage multiple services.

## Config

Services are defined in Sproc using a simple `services.toml` file (passed into Sproc using the `pin` command).

```toml
[services.example]
command = "example --a b"           # required
working_directory = "/home/example" # required

  [services.example.environment]    # optional
  EXAMPLE_ENV_VAR = "42"
```

You can inherit the services defined in other files using the `inherit` field. Inherited service files cannot expose an `inherit` field.

```toml
inherit = ["/path/to/other/services.toml", "/path/to/other/other/services.toml"]

[services.example]
# ...
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
