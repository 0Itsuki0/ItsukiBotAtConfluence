import { join } from 'path'
import { RustFunction } from 'cargo-lambda-cdk'
import { EndpointType, LambdaRestApi } from 'aws-cdk-lib/aws-apigateway'
import { Duration, Stack, StackProps } from "aws-cdk-lib"
import { Construct } from "constructs"
import { Effect, PolicyStatement } from 'aws-cdk-lib/aws-iam'
import { Queue } from 'aws-cdk-lib/aws-sqs'
import { SqsEventSource } from 'aws-cdk-lib/aws-lambda-event-sources'
import { Rule, Schedule } from 'aws-cdk-lib/aws-events'
import { LambdaFunction } from 'aws-cdk-lib/aws-events-targets'
import { namePrefix } from '../bin/cdk'



export class HandlerStack extends Stack {
    private contextKey = this.node.tryGetContext('context') ?? "develop"
    private context = this.node.tryGetContext(this.contextKey)

    private slackSigningSecret = this.context["SLACK_SIGNING_SECRET"]
    private chatModelId = this.context["CHAT_MODEL_ID"]
    private botToken = this.context["BOT_OAUTH_TOKEN"]
    private knowledgeBaseId: string = this.context["KNOWLEDGE_BASE_ID"]


    constructor(scope: Construct, id: string, props: StackProps) {
        super(scope, id, props)

        const queue = new Queue(this, `${namePrefix}SlackEventQueue.fifo`, {
            visibilityTimeout: Duration.minutes(10),
            fifo: true,
        })


        // apigateway lambda
        const apigatewayLambda = new RustFunction(this, `${namePrefix}APIGatewayLambda`, {
            manifestPath: join(__dirname, '..', '..', 'lambdas/receive_handler/Cargo.toml'),
            runtime: "provided.al2023",
            environment: {
                "SLACK_SIGNING_SECRET": this.slackSigningSecret,
                "QUEUE_URL": queue.queueUrl,
            }
        })

        queue.grantSendMessages(apigatewayLambda)

        const restApi = new LambdaRestApi(this, `${namePrefix}APIGateway`, {
            handler: apigatewayLambda,
            endpointTypes: [EndpointType.REGIONAL],
        })

        const sqsLambda = new RustFunction(this, `${namePrefix}SQSLambda`, {
            manifestPath: join(__dirname, '..', '..', 'lambdas/sqs_handler/Cargo.toml'),
            runtime: "provided.al2023",
            environment: {
                "SLACK_SIGNING_SECRET": this.slackSigningSecret,
                "QUEUE_ARN": queue.queueArn,
                "CHAT_MODEL_ID": this.chatModelId,
                "KNOWLEDGE_BASE_ID": this.knowledgeBaseId,
                "BOT_OAUTH_TOKEN": this.botToken
            },
            timeout: Duration.minutes(5)
        })

        queue.grantConsumeMessages(sqsLambda)
        sqsLambda.addEventSource(
            new SqsEventSource(queue, {
                batchSize: 1,
            })
        )

        sqsLambda.addToRolePolicy(new PolicyStatement({
            effect: Effect.ALLOW,
            actions: [
                'bedrock:*',
            ],
            resources: ['*'],
        }))

        const dailyLambda = new RustFunction(this, `${namePrefix}DailyDataSyncLambda`, {
            manifestPath: join(__dirname, '..', '..', 'lambdas/daily_data_sync_handler/Cargo.toml'),
            runtime: "provided.al2023",
            environment: {
                "KNOWLEDGE_BASE_ID": this.knowledgeBaseId,
            },
            timeout: Duration.minutes(5)
        });

        dailyLambda.addToRolePolicy(new PolicyStatement({
            effect: Effect.ALLOW,
            actions: [
                'bedrock:*'
            ],
            resources: ['*'],
        }))

        // 00:00 UTC on Weekdays
        const dailyRule = new Rule(this, `${namePrefix}DailyDataSyncRule`, {
            schedule: Schedule.cron({
                minute: '0',
                hour: '0',
                weekDay: 'MON-FRI',
            }),
            targets: [new LambdaFunction(dailyLambda, {
                retryAttempts: 0
            })]
        })

    }
}