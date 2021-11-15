import glob
import os

from fastapi import FastAPI
from fastapi.staticfiles import StaticFiles

import yaml


CONFIG_FILENAME = 'config.yaml'  # FIXME: use config

app = FastAPI(title='libTSN Configuration')
api = FastAPI(title="libTSN Web API")

app.mount('/api', api)
app.mount('/', StaticFiles(directory='build', html=True), name='front')


@api.get('/ifnames')
def read_ifnames():
    virtual_ifnames = {p.split(os.path.sep)[-1] for p in glob.glob('/sys/devices/virtual/net/*')}
    ifnames = {p.split(os.path.sep)[-1] for p in glob.glob('/sys/class/net/*')}
    return ifnames - virtual_ifnames


@api.get('/config/')
def read_root():
    with open(CONFIG_FILENAME) as f:
        config = yaml.load(f, Loader=yaml.FullLoader)

    return config


@api.put('/config/')
def update_item(item: dict):
    yaml.dump(item, open(CONFIG_FILENAME, 'w'), default_flow_style=False)
