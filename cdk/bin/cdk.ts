#!/usr/bin/env node
import 'source-map-support/register';
import * as cdk from 'aws-cdk-lib';
import { HandlerStack } from '../lib/handler-stack';

export const namePrefix = "ItsukiBotAtConfluence"

const app = new cdk.App();

const contextKey = app.node.tryGetContext('context') ?? "develop"
const context = app.node.tryGetContext(contextKey);
const region = context["AWS_REGION"] ?? process.env.CDK_DEFAULT_REGION


const handlerStack = new HandlerStack(app, `${namePrefix}HandlerStack`, {
    env: {
        region: region
    }
})
