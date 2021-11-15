import { Component } from 'react';

class Nic extends Component {
  render() {
    const { ifname, config } = this.props;
    return (
      <div>
        <h1>{ ifname }</h1>
        <div>
          { JSON.stringify(config) }
          { config.tas &&
            <>
              <h2>TAS</h2>
              <Tas data={ config.tas } />
            </>
          }
        </div>
      </div>
    );
  }
}

class Tas extends Component {
  constructor (props) {
    super(props);

    this.renderSchedule = this.renderSchedule.bind(this);
  }

  renderSchedule(schedule) {
    let headers = [<th>Time</th>];

    for (let i = -1; i < 8; i += 1) {
      headers.push(<th>{ i === -1 ? 'BE' : i }</th>);
    }

    let entries = schedule.map((entry, entryIndex) => {

      let prios = [];
      for (let prio = -1; prio < 8; prio += 1) {
        // TODO: editable
        prios.push(<td key={`${entryIndex}_${prio}`}><input type="checkbox" defaultChecked={entry.prio.includes(prio)} /></td>);
      }

      return (
        <tr key={entryIndex}>
          <td>{ entry.time }</td>
          { prios }
        </tr>
      );
    });

    return (
      <table>
        <thead>
          <tr>
            { headers }
          </tr>
        </thead>
        <tbody>
          { entries }
        </tbody>
      </table>
    );
  }

  render() {
    const { txtime_delay, schedule } = this.props.data;
    return (
      <div>
        <div>txtime_delay: { JSON.stringify(txtime_delay) }</div>
        <div>
          { this.renderSchedule(schedule) }
        </div>
      </div>
    );
  }
}

export { Nic };
