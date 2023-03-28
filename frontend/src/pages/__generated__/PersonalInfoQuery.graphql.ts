/**
 * @generated SignedSource<<e1c5b28a885d3fc6e6c072a62343c822>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest, Query } from 'relay-runtime';
export type PersonalInfoQuery$variables = {};
export type PersonalInfoQuery$data = {
  readonly currentUser: {
    readonly id: string;
    readonly primaryEmail: {
      readonly confirmedAt: any | null;
      readonly email: string;
      readonly id: string;
    } | null;
    readonly username: string;
  } | null;
};
export type PersonalInfoQuery = {
  response: PersonalInfoQuery$data;
  variables: PersonalInfoQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = {
  "alias": null,
  "args": null,
  "kind": "ScalarField",
  "name": "id",
  "storageKey": null
},
v1 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "User",
    "kind": "LinkedField",
    "name": "currentUser",
    "plural": false,
    "selections": [
      (v0/*: any*/),
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "username",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "concreteType": "UserEmail",
        "kind": "LinkedField",
        "name": "primaryEmail",
        "plural": false,
        "selections": [
          (v0/*: any*/),
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "email",
            "storageKey": null
          },
          {
            "alias": null,
            "args": null,
            "kind": "ScalarField",
            "name": "confirmedAt",
            "storageKey": null
          }
        ],
        "storageKey": null
      }
    ],
    "storageKey": null
  }
];
return {
  "fragment": {
    "argumentDefinitions": [],
    "kind": "Fragment",
    "metadata": null,
    "name": "PersonalInfoQuery",
    "selections": (v1/*: any*/),
    "type": "RootQuery",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "PersonalInfoQuery",
    "selections": (v1/*: any*/)
  },
  "params": {
    "cacheID": "526b0a2203f3fd6dff9e7268081c634a",
    "id": null,
    "metadata": {},
    "name": "PersonalInfoQuery",
    "operationKind": "query",
    "text": "query PersonalInfoQuery {\n  currentUser {\n    id\n    username\n    primaryEmail {\n      id\n      email\n      confirmedAt\n    }\n  }\n}\n"
  }
};
})();

(node as any).hash = "89c54ed3af708936d35fd9537ee223dd";

export default node;
