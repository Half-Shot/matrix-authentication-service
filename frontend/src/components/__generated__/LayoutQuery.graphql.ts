/**
 * @generated SignedSource<<99728a11c648b07d8865dca6b13bda08>>
 * @lightSyntaxTransform
 * @nogrep
 */

/* tslint:disable */
/* eslint-disable */
// @ts-nocheck

import { ConcreteRequest, Query } from 'relay-runtime';
export type LayoutQuery$variables = {};
export type LayoutQuery$data = {
  readonly currentUser: {
    readonly id: string;
    readonly username: string;
  } | null;
};
export type LayoutQuery = {
  response: LayoutQuery$data;
  variables: LayoutQuery$variables;
};

const node: ConcreteRequest = (function(){
var v0 = [
  {
    "alias": null,
    "args": null,
    "concreteType": "User",
    "kind": "LinkedField",
    "name": "currentUser",
    "plural": false,
    "selections": [
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "id",
        "storageKey": null
      },
      {
        "alias": null,
        "args": null,
        "kind": "ScalarField",
        "name": "username",
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
    "name": "LayoutQuery",
    "selections": (v0/*: any*/),
    "type": "RootQuery",
    "abstractKey": null
  },
  "kind": "Request",
  "operation": {
    "argumentDefinitions": [],
    "kind": "Operation",
    "name": "LayoutQuery",
    "selections": (v0/*: any*/)
  },
  "params": {
    "cacheID": "04a760c44feba28cd40c9ac34234c51c",
    "id": null,
    "metadata": {},
    "name": "LayoutQuery",
    "operationKind": "query",
    "text": "query LayoutQuery {\n  currentUser {\n    id\n    username\n  }\n}\n"
  }
};
})();

(node as any).hash = "7aed7e2d4ac8f64f4b0956b4061ad75e";

export default node;
