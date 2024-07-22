# xsu-cliff

Application web UI.

## Install

```bash
git clone https://github.com/hkauso/xsu
cd xsu
just build-s xsu-cliff sqlite
sudo mv target/release/xsu-cliff /usr/bin/xsu-cliff
```

## Update

```bash
sproc kill-all
cd xsu
git pull
just build-s xsu-cliff sqlite
sudo rm /usr/bin/xsu-cliff
sudo mv target/release/sproc /usr/bin/sproc
```
