import { Component } from 'react';
import Tas from './Tas';

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


class Cbs extends Component {
  constructor(props) {
    super(props);

    this.state = {
      config: props.data,
    };
  }

  available_classes = [
    '',
    'a',
    'b',
  ];

  render() {
    const config = this.state.config;

    const classes = [];
    const speeds = [];
    const max_frames = [];

    const selects = this.available_classes.map((cls) => <option>{cls}</option>);

    for (let i = -1; i < 8; i += 1) {
      if (config[i] === undefined) {
        classes.push(<td><select>{ selects }</select></td>);
        speeds.push(<td><input size="10" /></td>);
        max_frames.push(<td><input size="10" /></td>)
      } else {
        const cbs_config = config[i];
        classes.push(<td><select value={ cbs_config.class }>{ selects }</select></td>);
        speeds.push(<td><input className="number" size="10" value={ cbs_config.bandwidth } /></td>);
        max_frames.push(<td><input className="number" size="10" value={ cbs_config.max_frame } /></td>)
      }
    }

    return (
      <>
        <thead>
          <tr>
            <th>CBS</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <th>Class</th>
            { classes }
          </tr>
          <tr>
            <th>Speed</th>
            { speeds }
          </tr>
          <tr>
            <th>Max frame size</th>
            { max_frames }
          </tr>
        </tbody>
      </>
    );
  }
}

export { Nic };
