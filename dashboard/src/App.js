import React, { Component } from 'react';

import Nic from './Nic';
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
      config: { nics: {} },
      ifnames: [],
      currentIfname: '',
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

  setCurrentIfname(ifname) {
    this.setState({
      currentIfname: ifname,
    });
  }

  updateNic = (ifname, nicConfig) => {
    console.log(ifname, nicConfig);
    const { config } = this.state;
    config.nics[ifname] = nicConfig;

    console.log(config);

    this.setState({ config });
  }

  render() {
    const { config, ifnames, currentIfname } = this.state;

    if (!this.state.initialised) {
      return (<div>Loading...</div>);
    }

    return (
      <div className="App">
        <div className="nics-menu">
          {ifnames.map((ifname) => {
            return (
              <div className="nic" key={ifname} onClick={() => this.setCurrentIfname(ifname)}>
                {ifname}
              </div>
            );
          })}
        </div>

        { currentIfname &&
        <Nic key={ currentIfname }
          ifname={ currentIfname }
          update={ (data) => this.updateNic(currentIfname, data) }
          config={ config.nics[currentIfname] } />  }

        <div className="debug">
          <pre>{ JSON.stringify(config, null, 2) }</pre>
        </div>
      </div>
    );
  }
}

export default App;
