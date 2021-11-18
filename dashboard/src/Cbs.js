import { Component } from 'react';

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

  changeClass = (prio, cls) => {
    const config = this.state.config;
    if (cls === '') {
      delete config[prio];
    } else {
      if (config[prio] === undefined) {
        config[prio] = {};
      }
      config[prio].class = cls;
    }

    this.setState({ config });
    this.props.update(config);
  }

  changeSpeed = (prio, speed) => {
    const config = this.state.config;
    if (config[prio] === undefined) {
      config[prio] = {};
    }
    config[prio].speed = speed;

    this.setState({ config });
    this.props.update(config);
  }

  changeMaxFrame = (prio, max_frame) => {
    const config = this.state.config;
    if (config[prio] === undefined) {
      config[prio] = {};
    }
    config[prio].max_frame = max_frame;

    this.setState({ config });
    this.props.update(config);
  }

  render() {
    const config = this.state.config;

    const classes = [];
    const speeds = [];
    const max_frames = [];

    const selects = this.available_classes.map((cls) => <option key={cls}>{cls}</option>);

    for (let prio = -1; prio < 8; prio += 1) {
      const cbs_config = config[prio];
      let cls, speed, max_frame;
      if (cbs_config && cbs_config.class !== '') {
        cls = cbs_config.class;
        speed = cbs_config.speed;
        max_frame = cbs_config.max_frame;
      }

      classes.push(<td key={`cls-${prio}`}><select value={ cls } onChange={ e => this.changeClass(prio, e.target.value) }>{ selects }</select></td>);
      speeds.push(<td key={`speed-${prio}`}><input className="number" size="10" value={ speed } onChange={ e => this.changeSpeed(prio, e.target.value) } disabled={!cbs_config} /></td>);
      max_frames.push(<td key={`maxframe-${prio}`}><input className="number" size="10" value={ max_frame } onChange={ e => this.changeMaxFrame(prio, e.target.value) } disabled={!cbs_config} /></td>)
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

export default Cbs;
