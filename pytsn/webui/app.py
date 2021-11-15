import glob
import os

from fastapi import FastAPI
from fastapi.staticfiles import StaticFiles
from fastapi.middleware.cors import CORSMiddleware

from pydantic import BaseSettings

import yaml


class Settings(BaseSettings):
    CONFIG_FILENAME: str = "config.yaml"


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
    yaml.dump(item, open(settings.CONFIG_FILENAME, 'w'), default_flow_style=False)
