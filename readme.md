

Yara
=======

Yara is a companion tool for [ComfyUI](https://github.com/comfyanonymous/ComfyUI), based in the terminal. 
It can:

- **Pause queue generations by saving/loading them to files**
- **Cancel queued generations by their number/ID**
- **Examine the prompts and models in the running/pending queue**
- **Quickly regenerate many images with certain changes (specified nodes unmuted, muted, or replaced by LoadImage nodes)**
- **Toggle sleep mode, to prevent your computer from going to sleep and halting ComfyUI**
- **Hold your terminal until all jobs/prompts are finished**
- **Create an always-on-top window to display the latest generated image**
- **Display an image's embedded generation data**
- **Help download from CivitAI**


### [Installation](#installing) // [Usage](#usage)


</br>

# Installing

### [Direct link to download](https://github.com/Satellile/yara/releases/download/latest/yara.exe)

Download "yara.exe". 
Open a terminal in the same directory/folder as "yara.exe", and run the program through the terminal by simply typing `yara`.

The first time you run, you must select your ComfyUI output folder, and then a config file will automatically be created.
You can open the folder containing the config file with the argument `yara config`, to edit it manually (most of the options are just for configuring `yara preview`).

</br></br>

# Usage
1. [Saving, Loading, Deleting, and Listing Queues](#saving-queues)
2. [Examining the Running Queue](#examining-the-running-queue)
3. [Deleting Generations by Number](#deleting-generations-by-number)
4. [Regenerating Images With Modifications](#regenerating-images-with-modifications)
5. [Toggle Sleep Mode](#toggle-sleep-mode)
6. [Check an Image's Embedded Generation Info](#imagegen)
7. [Create a Window Displaying the Most Recently Generated Image](#create-a-window-displaying-the-most-recently-generated-image)
8. [Open the Folder Containing the Config File](#open-the-folder-containing-the-config-file)
9. [Download From CivitAI](#download-from-civitai)
10. [Print Help](#print-help)
11. [Aliases](#aliases)

## Saving, Loading, Deleting, and Listing Queues <a name="saving-queues"></a>

To save pending generations to a file, run

    yara save [name]

You can alternatively use "yara save -wr [name]" instead, if you wish to additionally save the currently active/in-progress generation. Note that Yara cannot save a partial generation; saving and loading an in-progress job will restart the generation from the beginning.

Now, you may clear out the queue or close ComfyUI. When you later want to resume generation, queue them up again by running
 
    yara load [name]

After the prompts have been sent to ComfyUI, Yara will wait for ComfyUI to finish generating the images before embedding workflow data into them. If you exit Yara before all images have generated, the loaded images won't have workflow metadata; you can fix this by using `yara fix`.

You can print out a list of all saved queues by typing

    yara list

and you can delete a saved queue with 

    yara delete [name]



## Examining the Running Queue


To print out the IDs of all queued prompts, as well as their model(s), lora(s), and positive prompt text, run

    yara examine

This can be useful if you load up a long queue, but forget the details of them. Or, if you messed up some of the prompts (.e.g forgot to remove a lora) and want to know which prompt ID's to delete, this can help.



## Deleting Generations by Number

To cancel queued generations, run

    yara cancel [prompt IDs]

where [prompt IDs] is a space-separated list of prompt IDs (the incrementing numbers labeling queues when you use "See Queues" in ComfyUI). 

You can also append "+" to a prompt ID to cancel that prompt as well as the next 100 prompts up, or specify an inclusive range of prompts using "-" as a separator. 

    yara cancel 60+       // Cancel generations 60, 61, 62, ..., 157, 158, 159
    
    yara cancel 25-30     // Cancel generations 25, 26, 27, 28, 29, 30
    
Deleting many prompts in ComfyUI is cumbersome. When you accidentally queue prompts with incorrect parameters or no longer care about a portion of the queue, this will make partial cancellation much faster and easier.



## Regenerating Images With Modifications

This feature allows you to quickly regenerate many images with certain changes to the workflow.

Node titles are used to specify which nodes to change and what changes to make. In ComfyUI, set a custom title (right click a node -> "Title") on the node you want to change, and edit it to include one of Yara's commands.

Available commands are:

| Command                   | Alias |  Function      | Limitations                                                    |
|---------------------------|-------------------------------------------|-----------------------------|--------------------------------------------|
| !yara_unmute | !yum | Unmute this node. | Only available for KSampler, KSamplerAdvanced, and SamplerCustom nodes.
| !yara_mute | !ym | Mute this node. | 
| !yara_load_here | !ylh | Replace this node with a LoadImage node, loading the original generated image | Only available on nodes outputting "IMAGE"

Generate the image in ComfyUI. When you want to regenerate it with the nodes modified, run

    yara regen [filepath]

to regenerate the image. You can include multiple files separated by spaces; I'd recommend selecting images in a file browser and dragging/dropping into the terminal window.

Alternatively, you can move the images you wish to regenerate into a folder, and simply run 

    yara regen

without any arguments, to regenerate every image in that folder. This folder that Yara looks for images in can be customized in the config file (`yara config`). By default, it looks in `ComfyUI/output/regen`.

This feature is mainly intended for 2-pass workflows (where you generate a low-res image, upscale it, and do another KSampler pass with low denoising). You can mute the second sampler and set `!yara_unmute` on it, generate many low-res images, and select the one(s) you want to do the second pass on. Then, instead of manually loading the image workflow and unmuting the node before regenerating, simply send the image through `yara regen`. You can also set `!yara_load_here` on the VAEEncode leading to the second sampler, and `!yara_mute` on the SaveImage output from the first sampler, to avoid regenerating the low-res image, saving time if you're on CPU or an old GPU.

To work, this feature requires ComfyUI workflow metadata embedded into the original image. Thus, it may not function with images generated through 3rd-party tools.

A small additional feature - "!yara_load_here" normally loads in the original image. However, if there is another image file in the same directory, with the same filename except with "edit" appended to the end (e.g. `ComfyUI_00022_.png` => `ComfyUI_00022_edit.png`), it will load that image instead, while using the original image's modified workflow data. This is useful if you're modifying the low-res image (such as to remove artifacts or improve composition) before you send it through the second sampler.



## Toggle Sleep Mode

ComfyUI doesn't prevent Windows from sleeping, but sleep mode halts ComfyUI generations. You can use yara to conveniently toggle sleep mode with 

    yara caffeine   // disable sleep mode
    yara melatonin  // enable sleep mode
    
By default, 'melatonin' will have Windows sleep after 30 minutes of inactivity. You can customize this length in the config file.



## Halt Terminal Until Queue Is Empty

To hold the terminal until the ComfyUI queue is empty, run

    yara wait

This is mostly useful just for halting the terminal until ComfyUI generations are done. Often, I disable sleep mode, then chain 'yara wait' with 'yara melatonin'. This lets me queue up a bunch of generations, and go leave my computer - when ComfyUI is finished running, sleep mode will be re-enabled, so my computer won't be running needlessly. I also might use this to execute other commands once ComfyUI is finished, such as if I want to generate images and train a LorA overnight, but don't want both to be running simultaneously.

As a shorthand, you can use 

    yara cwm

to disable sleep mode, wait until the queue is empty, and then re-enable sleep mode. (cwm standing for Caffeine/Wait/Melatonin).


## Check an Image's Embedded Generation Info  <a name="imagegen"></a>

Run 

    yara image

to start an interactive session. Enter the filepath of an image to obtain the generation data of the image.
Model(s), LorA(s), positive prompt text(s), and negative prompt text(s) will be printed to the screen, while the complete generation data will be copied to your clipboard with nice formatting.

(note: you can just drag/drop the image into the terminal window, and it will automatically input the image's filepath).



## Create a Window Displaying the Most Recently Generated Image

If you want to preview the generation output without having the ComfyUI window open, you can run

    yara preview

to open an always-on-top window that automatically displays the most recently generated image. Settings to configure the window location/size, or to toggle always-on-top/mouse passthrough and more are available in the config file (`yara config`).


## Open the Folder Containing the Config File

To open the folder containing the config.json file, run

    yara config


## Download From CivitAI

To download models/loras/etc from CivitAI, run

    yara cai [URLs]

where [URLs] is a space-separated list of the URLs of the CivitAI models/loras/etc
It will open a browser window to download them, and will copy the title, URL, filename, keywords, and description to your clipboard.

I mostly use it for the latter feature, as I keep a text file with relevant information for LorAs and this makes it easy to copy/paste all the key info. If you only want to copy the information to your clipboard, without downloading anything, add the '-nd' flag:

    yara cai -nd [URLs]


## Print Help

To display available commands/arguments, use

    yara help


## Aliases

Some of the commands can be shortened, for convenience:

| Command                   | Alias                                                                                                        |
|---------------------------|--------------------------------------------------------------------------------------------------------------------|
| save | s |
| load | l |
| delete | d |
| examine | e | 
| wait | w | 
| caffeine | c | 
| melatonin | m | 
| preview | p | 
| image | i | 
| help | h | 
| regen | rg |







# Other

If you have an issue, question, or request for some feature/config option, feel free to make an issue or message me.

This is developed mainly with Windows in mind. There's a Linux release, but it's missing some features (sleep mode toggles) and when I very briefly tested it, the image preview feature didn't work. I mostly use Windows and Linux downloads were a very small percentage of total downloads, so it's not something I'm prioritizing. If anybody wants to use it on Linux, feel free to make a pull request, a GitHub issue, or just send me a message so I know people are interested in it.

This is built for the latest ComfyUI release binary as of January 31, 2024. Future ComfyUI versions may change the API and thus break parts of this program.
