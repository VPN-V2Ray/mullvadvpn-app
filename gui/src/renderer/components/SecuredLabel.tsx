import * as React from 'react';
import { Component, Styles, Text, Types } from 'reactxp';
import { gettext } from '../../shared/gettext';

export enum SecuredDisplayStyle {
  secured,
  blocked,
  securing,
  unsecured,
}

interface IProps {
  displayStyle: SecuredDisplayStyle;
  style: Types.TextStyleRuleSet;
}

const styles = {
  securing: Styles.createTextStyle({
    color: 'rgb(255, 255, 255)', // white
  }),
  secured: Styles.createTextStyle({
    color: 'rgb(68, 173, 77)', // green
  }),
  unsecured: Styles.createTextStyle({
    color: 'rgb(208, 2, 27)', // red
  }),
};

export default class SecuredLabel extends Component<IProps> {
  public render() {
    return <Text style={[this.props.style, this.getTextStyle()]}>{this.getText()}</Text>;
  }

  private getText() {
    switch (this.props.displayStyle) {
      case SecuredDisplayStyle.secured:
        return gettext('SECURE CONNECTION');

      case SecuredDisplayStyle.blocked:
        return gettext('BLOCKED CONNECTION');

      case SecuredDisplayStyle.securing:
        return gettext('CREATING SECURE CONNECTION');

      case SecuredDisplayStyle.unsecured:
        return gettext('UNSECURED CONNECTION');
    }
  }

  private getTextStyle() {
    switch (this.props.displayStyle) {
      case SecuredDisplayStyle.secured:
      case SecuredDisplayStyle.blocked:
        return styles.secured;

      case SecuredDisplayStyle.securing:
        return styles.securing;

      case SecuredDisplayStyle.unsecured:
        return styles.unsecured;
    }
  }
}
