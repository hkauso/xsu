# 🎈 xsu-origami

`xsu-origami` is a simple binary which interprets a TOML-based file structure into HTTP routes.

## Usage

```bash
ori /path/to/project/root
```

Origami will automatically read a `routes.toml` file which is located in the project root. This file details information about each of your routes (relative to the root).

```toml
port = 8080

[env]
# the env holds variables that can be accessed by any route
hello = "world"

[router]

# routes.toml
[[routes]]
path = "/" # index
load = "routes/index.toml" # homepage route we defined; relative to the project root
method = "GET"

[[routes]]
path = "/api/ping"
load = "routes/ping.toml" # custom route we defined
method = "POST"

[[data]]
# `data` we can define data that should be sent in the body to this POST request
# everything is expected to be a string
# these are just field names which will be deserialized into `HashMap<String, String>`
order = 0 # or any number, this marks this field's spot when we fill it (see next example block)
name = "message"
type = "required" # or optional
# everything sent into data can be used in the SQL file the route uses AS WELL AS in the `response` field of the endpoint
```

Below is a simple example of the `routes/ping.toml` file we used in the previous example:

```toml
# routes/ping.toml
# everything we received in the `data` is filled in order with the `order` field that was defined in the `data` field of the route definition
[sql]
# the `sql` field allows you to execute sql
execute = "sql/ping_insert.sql" # everything from data is filled using a MYSQL-like "?" syntax
# you can also use: `fetch_one`, `fetch_all`
# `execute` returns no data

[modules]
authman = true # this will require the user to have a valid token from `xsu-authman`; this gives access to the `auth` field in the SQL/body

[response]
# here we can define information about how our route will respond
status = 200
body = "Message: ?"

  [reponse.headers]
  Content-Type = "text/plain"
```
