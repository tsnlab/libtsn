import React, { Component } from 'react';

import { Nic } from './Config';
import './App.css';

const API_BASE_URL = (
  (process.env.NODE_ENV === 'development') ?
  'http://localhost:8000/api' :
  '/api'
);


class App extends Component {
  constructor(props) {
    super(props);
    this.state = {
      config: {},
      ifnames: [],
      initialised: false,
    };
  }

  async componentDidMount() {
    const ifnames = fetch(`${API_BASE_URL}/ifnames`).then(res => res.json());
    const config = fetch(`${API_BASE_URL}/config`).then(res => res.json());

    console.debug(`Using ${API_BASE_URL}`);

    Promise.all([ifnames, config]).then(([ifnames, config]) => {
      this.setState({
        ifnames,
        config,
        initialised: true,
      });
    });
  }

  render() {
    const { config, ifnames } = this.state;

    if (!this.state.initialised) {
      return (<div>Loading...</div>);
    }

    return (
      <div className="App">
        <h1>Available NICs</h1>
        {ifnames.map((ifname, index) => {
          return (
            <div key={ifname}>
              <span>{index}: {ifname}</span>
            </div>
          );
        })}

        {Object.entries(config.nics).map(([key, value]) => {
          return (
            <Nic key={key} ifname={key} config={value} />
          );
        })}
      </div>
    );
  }
}

export default App;
