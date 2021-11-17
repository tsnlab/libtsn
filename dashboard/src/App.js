import React, { Component } from 'react';
import { MenuContainer, MenuItem, SubmitButton, Debug } from './Components';

import Nic from './Nic';
import './App.scss';

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
        <MenuContainer>
          {ifnames.map((ifname) => {
            return (
              <MenuItem key={ifname} className={ ifname === currentIfname && 'selected' } onClick={() => this.setCurrentIfname(ifname)}>
                {ifname}
              </MenuItem>
            );
          })}
        </MenuContainer>

        { currentIfname &&
        <Nic key={ currentIfname }
          ifname={ currentIfname }
          update={ (data) => this.updateNic(currentIfname, data) }
          config={ config.nics[currentIfname] } />  }

        <SubmitButton>Save</SubmitButton>

        <Debug>
          <h3>Preview</h3>
          <pre>{ JSON.stringify(config, this.jsonFormatter, 2) }</pre>
        </Debug>
      </div>
    );
  }
}

export default App;
