import { Component } from 'react';

class Nic extends Component {
  constructor(props) {
    super(props);
    this.state = {
      config: {},
    };
  }

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
          <table>
            <thead>
              { headers }
            </thead>
            <Tas data={ config.tas || {} } />
            <Cbs data={ config.cbs || {} } />
          </table>
        </div>
      </div>
    );
  }
}

class Tas extends Component {
  constructor (props) {
    super(props);

    this.state = {
      txtime_delay: props.data.txtime_delay,
      schedule: props.data.schedule,
    };

    this.renderSchedule = this.renderSchedule.bind(this);
  }

  async updateTxtime(txtime_delay) {
    const newState = {
      txtime_delay,
      schedule: this.state.schedule,
    }

    this.setState(newState);

    this.props.update(newState);
  }

  renderSchedule(schedule) {
    let entries;
    if (!schedule) {
      entries = [];
    } else {
      entries = schedule.map((entry, entryIndex) => {

        let prios = [];
        for (let prio = -1; prio < 8; prio += 1) {
          // TODO: editable
          prios.push(<td key={`${entryIndex}_${prio}`}><input type="checkbox" defaultChecked={entry.prio.includes(prio)} /></td>);
        }

        return (
          <tr key={entryIndex}>
            <td><input className="number" size="10" value={ entry.time } /></td>
            { prios }
          </tr>
        );
      });
    }

    let newPrios = Array(9).fill(<td><input type="checkbox" /></td>);

    return (
        <>
          { entries }
          <tr>
            <td><input className="number" size="10" /></td>
            { newPrios }
          </tr>
        </>
    );
  }

  render() {
    const { txtime_delay, schedule } = this.props.data;
    return (
      <>
        <thead>
          <tr>
            <th>TAS</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td colSpan="100%">txtime_delay: <input value={ txtime_delay } /></td>
          </tr>
          { this.renderSchedule(schedule) }
        </tbody>
      </>
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
