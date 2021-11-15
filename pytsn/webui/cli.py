from . import app
import uvicorn


def main():
    uvicorn.run(app.app)
