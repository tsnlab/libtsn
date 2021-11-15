from fastapi import FastAPI
from fastapi.staticfiles import StaticFiles

from .. import vlan

app = FastAPI(title='libTSN Configuration', docs_url='/api/docs/')
api = FastAPI()

app.mount('/api', api)
app.mount('/', StaticFiles(directory='build', html=True), name='front')


@api.get('/config/')
def read_root():
    return {}


@api.put('/config/')
def update_item(item: dict):
    return item
