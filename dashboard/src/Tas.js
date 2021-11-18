import { Component } from 'react';
import { NumberInput } from './Components';

class Tas extends Component {
  constructor (props) {
    super(props);

    this.state = {
      txtime_delay: props.data.txtime_delay || 0,
      schedule: props.data.schedule || [],
    };
  }

  onChangeTxtime = (e) => {
    const txtime_delay = parseInt(e.target.value || 0);
    this.setState({
      txtime_delay,
    });

    this.props.update({
      txtime_delay,
      schedule: this.state.schedule,
    })
  };

  changeGate = (slotIndex, prio, value) => {
    const { schedule } = this.state;
    const prios = new Set(schedule[slotIndex].prio);
    if (value) {
      prios.add(prio);
    } else {
      prios.delete(prio);
    }

    schedule[slotIndex].prio = Array.from(prios);

    this.setState({
      schedule,
    });

    this.props.update({
      txtime_delay: this.state.txtime_delay,
      schedule,
    });
  };

  addTimeslot = () => {
    const { schedule } = this.state;
    schedule.push({
      prio: [],
      time: 0,
    });

    this.setState({
      schedule,
    });

    // TODO: update?
  };

  changeSlotTime = (slotIndex, value) => {
    const { schedule } = this.state;
    if (schedule[slotIndex] === undefined) {
      schedule[slotIndex] = {
        prio: [],
        time: value,
      };
    } else {
      schedule[slotIndex].time = value;
    }

    this.setState({
      schedule,
    });

    this.props.update({
      txtime_delay: this.state.txtime_delay,
      schedule,
    });
  };

  renderSchedule = (schedule) => {
    let entries;
    if (!schedule) {
      entries = [];
    } else {
      entries = schedule.map((entry, entryIndex) => {

        let prios = [];
        for (let prio = -1; prio < 8; prio += 1) {
          prios.push(<td key={`${entryIndex}_${prio}`}><input type="checkbox" checked={entry.prio.includes(prio)} onChange={ (e) => { this.changeGate(entryIndex, prio, e.target.checked) } } /></td>);
        }

        return (
          <tr key={entryIndex} data-key={entryIndex}>
            <td><NumberInput key={ `${entryIndex}-input` } size="10" value={ entry.time } onChange={ (e) => this.changeSlotTime(entryIndex, e.target.value) } /></td>
            { prios }
          </tr>
        );
      });
    }

    return (
        <>
          { entries }
          <tr key={ entries.length } data-key={ entries.length }>
            <td><button onClick={this.addTimeslot}>Add timeslot</button></td>
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
          </tr>
        </thead>
        <tbody>
          <tr>
            <td colSpan="100%">txtime_delay: <NumberInput value={ txtime_delay } onChange={this.onChangeTxtime} /></td>
          </tr>
          { this.renderSchedule(schedule) }
        </tbody>
      </>
    );
  }
}

export default Tas;
