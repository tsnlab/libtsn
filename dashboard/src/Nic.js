import { Component } from 'react';
import Tas from './Tas';
import Cbs from './Cbs';

class Nic extends Component {
  constructor(props) {
    super(props);
    this.state = {
      config: props.config,
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
    const { ifname, config } = this.props;

    let headers = [ <th>Option</th> ];

    for (let i = -1; i < 8; i += 1) {
      headers.push(<th>{i === -1 ? 'BE' : i}</th>);
    }

    return (
      <div>
        <h1>{ ifname }</h1>
        <div className="schedulers">
          <div>{ JSON.stringify(this.state.config) }</div>
          <table>
            <thead>
              { headers }
            </thead>
            <Tas data={ config.tas || {} } update={ this.updateTas } />
            <Cbs data={ config.cbs || {} } update={ this.updateCbs } />
          </table>
        </div>
      </div>
    );
  }
}

export default Nic;
