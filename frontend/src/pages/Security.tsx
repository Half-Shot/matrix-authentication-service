// Copyright 2022 The Matrix.org Foundation C.I.C.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import {
  Box,
  Button,
  Card,
  CardActions,
  CardContent,
  CardMedia,
  Icon,
  Skeleton,
  Typography,
} from "@mui/material";
import { graphql, useLazyLoadQuery } from "react-relay";

import BrowserSessionList from "../components/BrowserSessionList";
import CompatSsoLoginList from "../components/CompatSsoLoginList";
import OAuth2SessionList from "../components/OAuth2SessionList";
import { NavLink } from "react-router-dom";
import {
  AppsTwoTone,
  ContactMailTwoTone,
  LockTwoTone,
} from "@mui/icons-material";
import { SessionsQuery } from "./__generated__/SessionsQuery.graphql";
import { SecurityQuery } from "./__generated__/SecurityQuery.graphql";

const Security: React.FC = () => {
  const data = useLazyLoadQuery<UserSecurityQuery>(
    graphql`
      query UpstreamQuery($count: Int!, $cursor: String) {
        currentUser {
          id
          passwordSetAt
        }
      }
    `,
    { count: 2 }
  );

  const userData = useLazyLoadQuery<UserQuery>(
    graphql`
      query UserQuery($count: Int!, $cursor: String) {
        currentUser {
          id
          passwordSetAt
        }
      }
    `,
    { count: 2 }
  );

  if (data.currentBrowserSession) {
    const session = data.currentBrowserSession;
    const user = session.user;

    return (
      <>
        <Typography variant="h4">Hello {user.username}!</Typography>
        <div className="mt-4 grid lg:grid-cols-3 gap-1">
          <OAuth2SessionList user={user} />
          <CompatSsoLoginList user={user} />
          <BrowserSessionList user={user} currentSessionId={session.id} />
        </div>
      </>
    );
  } else {
    return <Skeleton />;
  }
};

export default Security;
