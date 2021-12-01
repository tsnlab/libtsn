import uvicorn

from . import app


def main():
    uvicorn.run(app.app)
