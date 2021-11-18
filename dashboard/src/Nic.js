import { Component } from 'react';
import Tas from './Tas';
import Cbs from './Cbs';

class Nic extends Component {
  constructor(props) {
    super(props);
    let config = props.config;
    if (config === undefined) {
      config = {
        tas: {},
        cbs: {},
        'egress-qos-map': {},  // TODO: Make this properly
      };
    }
    this.state = {
      ifname: props.ifname,
      config,
    };
  }

  updateTas = (value) => {
    let config = {...this.state.config };
    config.tas = value;
    this.setState({
      config,
    });

    this.props.update(config);
  };

  updateCbs = (value) => {
    let config = {...this.state.config };
    config.cbs = value;
    this.setState({
      config,
    });

    this.props.update(config);
  };

  render() {
    const { ifname, config } = this.state;

    let headers = [ <th key="option">Option</th> ];

    for (let prio = -1; prio < 8; prio += 1) {
      headers.push(<th key={prio}>{prio === -1 ? 'BE' : prio}</th>);
    }

    return (
      <div>
        <h1>{ ifname }</h1>
        <div className="schedulers">
          <table>
            <thead>
              <tr>
                { headers }
              </tr>
            </thead>
            <Tas key="tas" data={ config.tas || {} } update={ this.updateTas } />
            <Cbs key="cbs" data={ config.cbs || {} } update={ this.updateCbs } />
          </table>
        </div>
      </div>
    );
  }
}

export default Nic;
