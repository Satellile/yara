

    Yara
=======

Yara is a companion tool for [ComfyUI](https://github.com/comfyanonymous/ComfyUI), based in the terminal. 


Yara can:

- **Pause queue generations via saving/loading them to files**
- **Delete batches of queued generations by their number/ID**
- **Examine the prompts and models in the running/pending queue**
- **Toggle sleep mode, to prevent your computer from going to sleep and stopping ComfyUI**
- **Wait until all jobs/prompts are finished, estimating the remaining time**
- **Create a small always-on-top window that displays the latest generated image**
- **Check the embedded generation data in an image**
- **Help download from CivitAI**


### [Installation](#installing)
### [Usage](#usage)
### [Aliases](#aliases)



# Installing

### [Direct link to download](https://github.com/comfyanonymous/ComfyUI/releases/download/latest/ComfyUI_windows_portable_nvidia_cu118_or_cpu.7z)

Download "yara.exe". 

Open a terminal in the same directory/folder as "yara.exe", and run yara through the terminal by typing "yara".

The first time you run, you must select your ComfyUI output folder, and then a config file will automatically be created.
You can open the folder containing the config file with the argument "yara config", to edit it manually (most of the options are just for configuring "yara preview").


# Usage

## Saving, Loading, Deleting, and Listing Queues

To save pending generations to a file, run

    yara save [name]

You can alternatively use "yara save -wr [name]" instead, if you wish to additionally save the currently active/in-progress generation. Note that Yara cannot save a partial generation; saving and loading an in-progress job will restart the generation from the beginning.

Now, you may clear out the queue or close ComfyUI. When you later want to resume generation, queue them up again by running
 
    yara load [name]

You can print out a list of all saved queues by typing

    yara list

and you can delete a saved queue with 

    yara delete [name]

**Warning**: After saving/loading generations, resulting images will not have the workflow embedded in them; i.e. you can no longer drag/drop them into ComfyUI to recreate the workflow. The generation details (prompt, model, loras, seed, etc) are still embedded within the image, though, and you can view that by either reading the image file as a text file, or using Yara's image-generation-info function ('yara image').

From [ComfyUI Github Issue #69](https://github.com/comfyanonymous/ComfyUI/issues/69), importing workflows from the api prompt format is planned, so this will hopefully be fixed later.



## Examining the Running Queue

Sometimes, I load up a long queue of generations, but forget what they were. Or, I may have messed up some of the prompts (e.g. forgot to remove a lora), and want to know which prompt ID's to delete. To examine the active queue, you can run

    yara examine

and it will print out the IDs of all prompts, as well as their model(s), lora(s), and positive prompt text.


## Deleting Generations by Number

When I mess up a bunch of prompts, I often want to delete many at once. Doing so in ComfyUI is cumbersome due to some UI issues, so you can instead do it here.
To cancel queued generations, run

    yara cancel [prompt IDs]

where [prompt IDs] is a space-separated list of prompt IDs (the incrementing numbers labeling queues when you use "See Queues" in ComfyUI). 

You can also append "+" to a prompt ID to cancel that prompt as well as the next 100 prompts up, or specify an inclusive range of prompts using "-" as a separator. 

    yara cancel 60+       // Cancel generations 60, 61, 62, ..., 157, 158, 159
    
    yara cancel 25-30     // Cancel generations 25, 26, 27, 28, 29, 30



## Toggle Sleep Mode

ComfyUI doesn't prevent Windows from sleeping, but sleep mode halts ComfyUI generations. You can use yara to conveniently toggle sleep mode with 

    yara caffeine   // disable sleep mode
    yara melatonin  // enable sleep mode

Melatonin will, by default, set Windows to sleep after 30 minutes. You can customize this length in the config file.


## Halt Terminal Until Queue Is Empty

Running 

    yara wait

will halt the terminal until the ComfyUI queue is empty. While waiting, it will print the number of remaining generations every five minutes. It also will estimate (incredibly roughly and naively) the amount of time until all generations are finished.

This is mostly useful for halting the terminal until ComfyUI generations are done. Often, I disable sleep mode, then chain 'yara wait' with 'yara melatonin'. This lets me queue up a bunch of generations, and go leave my computer - when ComfyUI is finished running, sleep mode will be re-enabled, so my computer won't be running needlessly. I also might use this to execute other commands once ComfyUI is finished, such as if I want to both generate images and train a LorA overnight, but don't want both to be running simultaneously.

As a shorthand, you can use 

    yara cwm

to disable sleep mode, wait until the queue is empty, and then re-enable sleep mode. (cwm standing for Caffeine/Wait/Melatonin).


## Check an Image's Embedded Generation Info

Run 

   yara image

to start an interactive session. Enter the filepath of an image, and it obtain the generation data of the image.
Model(s), LorA(s), positive prompt text(s), and negative prompt text(s) will be printed to the screen, while the full generation data will be copied to your clipboard.

(note: you can just drag/drop the image into the terminal window, and it will automatically input the image's filepath).

This is particularly useful since when the workflow isn't embedded into the image (as discussed in the "[Saving, Loading, Deleting, and Listing Queues](#usage)" section above).


## Create a Window Displaying the Most Recently Generated Image

If you want to preview the generation output without having the ComfyUI window open, you can run

    yara preview

to open an always-on-top window that automatically displays the most recently generated image. Settings to configure the window location/size, or to toggle always-on-top/mouse passthrough and more are available in the config file ('yara config').


## Open the Folder Containing the Config File

To open the folder containing the config.json file, run

    yara config


## Help download from CivitAI

Run 

    yara cai [URLs]

where [URLs] is a space-separated list of the URLs of the CivitAI models/loras/etc you want to download. It will open a browser window to download them, and will copy the title, URL, filename, keywords, and description to your clipboard.

I mostly use it for the latter feature, as I keep a text file with relevant information for LorAs and this makes it easy to copy/paste all the key info. If you only want to copy the information to your clipboard, without downloading anything, add the '-nd' flag:

    yara cai -nd [URLs]


## Print Commands

To display available commands/arguments, use

    yara help



# Aliases

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







This is built for the latest ComfyUI release as of July 22, 2023. Future updates may change the API and thus break parts of this program.



# Other

If you have an issue, question, or request for some feature/config option, feel free to make an issue or message me.

This is currently Windows-only. A linux build should be able to be compiled from source, though, I think. I mostly just don't feel like doing it since I don't think too many people are going to use this, but if anyone actually wants to use it and is on Linux, message me or make an issue post.
