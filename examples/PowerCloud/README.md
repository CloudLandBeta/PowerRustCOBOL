<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
-->

# PowerCloud

One AWS service on every tab of one window, written in RustCOBOL. It is an example for
**PowerRustCOBOL AI**: open it from **Help ▸ Examples ▸ PowerCloud**.

📷 Screenshot needed — `powercloud-settings.png`: the Settings tab right after opening the example,
with the services wheel on the left and the cloud-access fields on the right.

| Tab | What it does | AWS control it uses |
|---|---|---|
| **Settings** | Takes an AWS user's keys (kept in memory only), proves them with one read-only call, deploys the CloudFormation stack the demo needs or saves the template for you to run yourself. | `AwsMcp` (`AwsCall`) |
| **DynamoDB** | A table of pets with a photo each: create the table, filter the grid, add, view, edit and delete a pet in popups. The photo is cut to a 400 × 400 square, kept in S3 and shown through a temporary link. | `AwsDynamoDB`, `AwsS3`, `AwsLambda` |
| **S3** | Pick one of the account's buckets and read the objects in it. | `AwsS3` |
| **Lambda** | Explains the function that makes the temporary links and lets you call it. | `AwsLambda` |
| **Polly** | Type a sentence, pick a voice, listen to it with a play / pause / stop player. | `AwsPolly` |
| **EC2** | The servers of a region with their class, vCPU and memory, and an on / off column that starts or stops one while a Working window waits. | `AwsEC2` |
| **Rekognition** | Drop a photo; a box is drawn over every celebrity found, with their name and links beside it. | `AwsRekognition` |

## What you need

- **Nothing to install for most of it.** The DynamoDB, S3, Polly, EC2, Rekognition and CloudFormation parts reach
  AWS's hosted MCP service directly, over the internet. **`uvx`** (from [uv](https://docs.astral.sh/uv/)) is needed
  only for the Lambda tab and for the temporary photo links on the DynamoDB tab, because both call a Lambda
  function through the server program AWS publishes for it; its first start takes a minute because it downloads it.
- **An AWS account and an AWS user** whose keys you type into the Settings tab. Nothing is written to a
  file: the keys are kept in memory for this run and the boxes are emptied as soon as you press **Connect**.
  Leave them empty to use the default AWS credentials of the machine instead.
- To *deploy the stack from the Settings tab*, the user needs the rights CloudFormation asks for (it creates an
  IAM role and a policy). To *run the demo*, it needs only the managed policy the stack creates; attach the
  policy printed in the stack's **Outputs** (`OperatorPolicyArn`) to that user.

## The cloud side

`cloudformation/powercloud.yaml` creates a private, encrypted S3 bucket for the photos, the Lambda function
`powercloud-presign` that signs a short-lived link to one photo, the policy above, and — only if you ask —
the DynamoDB table and a small demo server. Two ways to deploy it:

1. **From the application**, with keys allowed to create it (an administrator's): Settings ▸ Connect, then
   **Deploy CloudFormation**. The tab shows the stack's progress and fills in the bucket and function names
   when it is done. Keys made only to run the demo are refused, and the tab says so.
2. **By yourself.** Press **Save template…** (or the link beside it) and run it with the AWS CLI or the
   console:

   ```
   aws cloudformation deploy --template-file powercloud.yaml --stack-name powercloud --capabilities CAPABILITY_IAM
   ```

Either way, **Connect** finds the stack named in **Stack name** — deployed now or on an earlier run — and
fills in the bucket and function names from its outputs. A bucket of your own can be typed into the
**Photo bucket** box instead; without a bucket, saving a pet with a photo says so rather than trying.

📷 Screenshot needed — `powercloud-pets.png`: the DynamoDB tab with a few pets, their 40 × 40 photos and the
View, Edit and Delete icons.

## Cost and cleaning up

The bucket, the function and the on-demand table cost nothing while nobody uses them. The demo server, if you
ask for one, is a `t3.micro` that bills while it runs: stop it from the EC2 tab. To remove everything, empty the
bucket first (CloudFormation will not delete a bucket that holds objects), then delete the stack.

## Things worth knowing

- **Six languages.** The list beside the close button (✕) switches the whole window, and the popups it opens,
  between English, Português, Español, Français, 日本語 and 中文 at once. The program opens in English and keeps
  the choice in memory for the run; nothing is written to a file. The texts are one table in the main form's
  working storage — a row per text, a column per language — and a status line remembers *which* message it
  showed and what went into it, so a line already on screen is written again in the new language. Three things
  are not the program's to translate: the picture of the services wheel, the `Filter…` fields the DataGrid
  draws under its headings, and the `Drop files here, or browse` line of the drop zones (the example's own hint
  under it is translated). A new text goes into the table in all six columns, with the English one first.
- **Regions.** The project has one AWS connection per region (`us-east-1`, `us-east-2`, `us-west-2`,
  `eu-west-1`, `eu-central-1`, `sa-east-1`). The Settings region picks the one the tabs use; the EC2 tab has
  a list of its own. Rekognition's celebrity recognition is not offered in every region.
- **The On / off column is a checkbox.** A DataGrid has Image, Button, Checkbox and Dropdown cells; a switch
  that looks like the Switch control is not one of them yet.
- **The wheel is a drawing made for this example.** `Assets/images/aws-services-wheel.webp` (800 × 900) holds
  no vendor logo. Put any picture of that size under the same name to replace it.
- **The popups.** The pet, view and confirm windows are forms of their own (`pet-form`, `view-form`,
  `confirm-form`) that talk to the main form through properties (`PetName`, `PetSaved`, `ConfirmAnswer` …); the
  Working window (`working-form`) is opened with `OpenFormAsync` and closed by the main form when the server
  has changed state.
- **No key is ever saved.** `PowerCloud.project.toml` names regions, never a key, and the project's tests
  scan every file of the example for one.
