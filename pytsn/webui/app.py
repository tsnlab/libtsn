import glob
import os

import yaml

from fastapi import FastAPI
from fastapi.middleware.cors import CORSMiddleware
from fastapi.staticfiles import StaticFiles
from pydantic import BaseSettings


class Settings(BaseSettings):
    CONFIG_FILENAME: str = "config.yaml"


def normalise_dict(obj: dict) -> dict:
    if not isinstance(obj, dict):
        return obj

    def make_integer(key):
        try:
            return int(key)
        except ValueError:
            return key

    return {
        make_integer(key): normalise_dict(val)
        for key, val in obj.items()
    }


def sanitise_configs(config: dict) -> dict:
    for nic, nicConfig in config['nics'].items():
        if nicConfig['cbs'] == {}:
            del nicConfig['cbs']

        if nicConfig['tas']['schedule'] == []:
            del nicConfig['tas']

    return config


settings = Settings()
app = FastAPI(title='libTSN Configuration')
api = FastAPI(title="libTSN Web API")

STATIC_PATH = os.path.join(os.path.dirname(__file__), 'static')

app.mount('/api', api)
app.mount('/', StaticFiles(directory=STATIC_PATH, html=True), name='front')

origins = [
    'http://localhost:3000',
]

app.add_middleware(
    CORSMiddleware,
    allow_origins=origins,
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)


@api.get('/ifnames')
def read_ifnames():
    virtual_ifnames = {p.split(os.path.sep)[-1] for p in glob.glob('/sys/devices/virtual/net/*')}
    ifnames = {p.split(os.path.sep)[-1] for p in glob.glob('/sys/class/net/*')}
    return ifnames - virtual_ifnames


@api.get('/config')
def read_root():
    with open(settings.CONFIG_FILENAME) as f:
        config = yaml.load(f, Loader=yaml.FullLoader)

    return config


@api.put('/config')
def update_item(item: dict):
    item = normalise_dict(item)
    item = sanitise_configs(item)
    yaml.dump(item, open(settings.CONFIG_FILENAME, 'w'), default_flow_style=False)
