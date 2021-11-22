import { Component } from 'react';
import Tas from './Tas';
import Cbs from './Cbs';

import { TextInput } from './Components';

class Nic extends Component {
  constructor(props) {
    super(props);
    let config = props.config;
    if (config === undefined) {
      config = {
        tas: {},
        cbs: {},
        vlan: {
          ipv4: null,
          maps: {},
        }
      };
    }

    let vlanid, ipv4;
    if (Object.keys(config.vlan).length > 0) {
      vlanid = Object.keys(config.vlan)[0];
      ipv4 = config.vlan[vlanid].ipv4;
    } else {
      console.log(Object.keys(config.vlan));
    }
    this.state = {
      ifname: props.ifname,
      config,
      vlanid,
      ipv4,
    };
  }

  updateConfig = (config) => {
    this.setState({
      config,
    });

    this.props.update(config);
  }

  regenerateVlan = () => {
    const config = this.state.config;
    const { ipv4, vlanid } = this.state;

    config.vlan = {};
    config.vlan[vlanid] = {
      ipv4,
      maps: Object.fromEntries([...Array(8).keys()].map(x => [x, x])),
    }

    this.updateConfig(config);
  }

  updateVlanId = (value) => {
    this.setState({
      vlanid: value,
    }, this.regenerateVlan);
  };

  updateIpv4 = (value) => {
    this.setState({
      ipv4: value,
    }, this.regenerateVlan);
  }

  updateTas = (value) => {
    let config = {...this.state.config };
    config.tas = value;

    this.updateConfig(config);
  };

  updateCbs = (value) => {
    let config = {...this.state.config };
    config.cbs = value;

    this.updateConfig(config);
  };

  render() {
    const { ifname, config } = this.state;

    let headers = [ <th key="option">Option</th> ];

    for (let prio = -1; prio < 8; prio += 1) {
      headers.push(<th key={prio}>{prio === -1 ? 'BE' : prio}</th>);
    }

    const { vlanid, ipv4 } = this.state;

    return (
      <div>
        <h1>{ ifname }</h1>
        <label>VLAN id:
          <TextInput value={vlanid} onChange={ e => this.updateVlanId(e.target.value) } />
        </label>
        <label> IPv4:
          <TextInput value={ipv4} onChange={ e => this.updateIpv4(e.target.value) } />
        </label>
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
