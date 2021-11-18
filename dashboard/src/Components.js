import styled from 'styled-components'

const NumberInput = styled.input`
  margin: 0 .5em;
  text-align: right;
  font-feature-settings: "tnum";
  font-variant-numeric: tabular-nums;
`

const TextInput = styled.input`
  margin: 0 .5em;
`

const MenuContainer = styled.div`
  display: flex;
  align-items: center;
  justify-content: center;
`

const MenuItem = styled.div`
  border-radius: 5px;
  border: 1px solid black;
  box-sizing: border-box;
  margin: 1em;
  padding: .5em 1em;

  &.selected {
    border-color: #ccc;
  }
`

const SubmitButton = styled.button`
  border-radius: 5px;
  border: 1px solid black;
  box-sizing: border-box;
  width: calc(100% - 2em);
  margin: 1em;
  padding: .5em 1em;
`

const Debug = styled.div`
  text-align: left;
`

export {
  NumberInput,
  TextInput,
  MenuContainer,
  MenuItem,
  SubmitButton,
  Debug,
}
