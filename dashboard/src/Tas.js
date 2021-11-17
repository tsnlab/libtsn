import { Component } from 'react';

class Tas extends Component {
  constructor (props) {
    super(props);

    this.state = {
      txtime_delay: props.data.txtime_delay,
      schedule: props.data.schedule,
    };
  }

  async updateTxtime(txtime_delay) {
    const newState = {
      txtime_delay,
      schedule: this.state.schedule,
    }

    this.setState(newState);
  }

  onChangeTxtime = (e) => {
    const data = parseInt(e.target.value || 0);
    this.setState({
      txtime_delay: data,
    });
  }

  renderSchedule = (schedule) => {
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
    const { txtime_delay, schedule } = this.state;
    return (
      <>
        <thead>
          <tr>
            <th>TAS</th>
            <td colSpan="100%">debug: { JSON.stringify(this.state)  }</td>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td colSpan="100%">txtime_delay: <input value={ txtime_delay } onChange={this.onChangeTxtime} /></td>
          </tr>
          { this.renderSchedule(schedule) }
        </tbody>
      </>
    );
  }
}

export default Tas;
