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

You can configure the server key and port in the `server` field:

```toml
[server]
port = 6374
key = "abcd"

# ...
```

The server is needed to start services that use the `restart` field. You can make services automatically restart (when spawned from the server) by setting `restart` to `true`:

```toml
[services.example]
command = "node index.js"
working_directory = "/home/example"
restart = true # this service will ONLY restart when started from the server
```

It is not recommended that you manually update the pinned `services.toml` file (`$HOME/.config/sproc/services.toml`). This file is regularly updated by the CLI and server, and manual changes should ONLY be done through `sproc pin`.

## Usage

Load config file:

```bash
sproc pin {path}
```

Start service:

```bash
sproc run {name}
```

Start a service in a new task (HTTP server required):

```bash
sproc spawn {name}
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

Start observation server:

```bash
sproc serve
```
